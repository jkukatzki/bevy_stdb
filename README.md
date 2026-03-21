# bevy_stdb

A [Bevy](https://bevy.org/) integration for [SpacetimeDB](https://spacetimedb.com).
[![crates.io](https://img.shields.io/crates/v/bevy_stdb)](https://crates.io/crates/bevy_stdb)
[![docs.rs](https://docs.rs/bevy_stdb/badge.svg)](https://docs.rs/bevy_stdb)

![Useless AI generated image that kind of looks cool](https://github.com/user-attachments/assets/b6cf0408-0c0d-4997-bf9c-e2e0989ab5f3)
_Please enjoy this useless AI generated image based on the README contents of this repo._



## Overview

`bevy_stdb` adapts SpacetimeDB's connection and callback model into Bevy-style resources, systems, plugins, and messages. Its built around a few core ideas:

- Configure everything through `StdbPlugin`
- Expose the active live connection as a Bevy resource via `StdbConnection`
- Forward table callbacks into Bevy messages with `TableRegistrar`
- Store subscription intent independently from the live connection with `StdbSubscriptions`
- Optionally retry failed connections with `StdbReconnectOptions`

The library is organized around connection-scoped lifecycle concerns:

- **connection lifecycle**: establish the initial connection, expose the active connection resource, and track connection state
- **table lifecycle**: initialize table message channels once and re-bind table callbacks whenever a connection becomes active
- **subscription lifecycle**: store desired subscription intent and re-apply queued subscriptions when connected
- **reconnect lifecycle**: optionally retry connection attempts after disconnects using configurable backoff

## Features

- **Builder-style setup** via `StdbPlugin`
- **Connection resource** access through `StdbConnection`
- **Table event bridging** into normal Bevy `Message`s
- **Managed subscription intent** through `StdbSubscriptions`
- **Optional reconnect support** through `StdbReconnectOptions`

## Example

```rust
use bevy::prelude::*;
use bevy_stdb::prelude::*;
use crate::module_bindings::{DbConnection, RemoteModule};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum MySubKey {
    PlayerInfo,
}

pub type StdbConn = StdbConnection<DbConnection>;
pub type StdbSubs = StdbSubscriptions<MySubKey, RemoteModule>;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(
            StdbPlugin::<DbConnection, RemoteModule>::default()
                .with_module_name("my_module")
                .with_uri("http://localhost:3000")
                .with_table(|reg| reg.add_table(&reg.db().player_info()))
                .with_table(|reg| reg.add_table_without_pk(&reg.db().nearby_monsters()))
                .with_subscriptions::<MySubKey>(|subs| {
                    // This is a great place to add "global" subscriptions, but you can subscribe from any system too.
                    subs.subscribe_query(MySubKey::PlayerInfo, |q| q.from.player_info());
                })
                .with_reconnect(StdbReconnectOptions::default())
                .with_run_fn(DbConnection::run_threaded),
        )
        .add_systems(Update, on_player_info_insert)
        .run();
}

fn on_player_info_insert(mut msgs: ReadInsertMessage<PlayerInfo>) {
    for msg in msgs.read() {
        info!("player inserted: {:?}", msg.row);
    }
}
```

## Table registration

Use `StdbPlugin::with_table` to register one table per call.

The closure receives a `TableRegistrar` that already carries the current database view. Use `reg.db()` to access that view and then register exactly one table.

Typical forms are:

```rust
.with_table(|reg| reg.add_table(&reg.db().player_info()))
.with_table(|reg| reg.add_table_without_pk(&reg.db().nearby_monsters()))
.with_table(|reg| reg.add_event_table(&reg.db().some_event_stream()))
.with_table(|reg| reg.add_view(&reg.db().some_view()))
```

These registrations are replayed during initialization to install the required Bevy message channels and again whenever the connection enters the connected state to bind callbacks for the current live database view.

## Messages

Depending on the table shape, `bevy_stdb` forwards updates into Bevy messages such as:

- `InsertMessage<T>`
- `DeleteMessage<T>`
- `UpdateMessage<T>`
- `InsertUpdateMessage<T>`

This lets normal Bevy systems react to database changes using message readers.

## Subscriptions

`StdbSubscriptions` stores desired subscription intent separately from the live connection.

That means you can:

- declare global (or any other) subscriptions during plugin setup with `with_subscriptions`
- queue additional subscriptions later from normal Bevy systems
- automatically re-apply queued subscription intent after reconnect

Subscriptions are keyed, so the application can refer to them using its own domain-specific identifiers, allowing you to also unsubscribe as needed later.

## Reconnects

Reconnect behavior is opt-in.

Use `StdbPlugin::with_reconnect` with `StdbReconnectOptions` to enable retry behavior after disconnects. When reconnect succeeds:

- the live `StdbConnection` resource is replaced
- table callbacks are re-bound
- queued subscriptions are re-applied

## Compatibility

| bevy_stdb | bevy   | spacetimedb_sdk |
| --------- | ------ | --------------- |
| 0.1.0     | 0.18.x | 2.0.x           |

## Notes

This crate focuses on table-driven client workflows. Reducer and procedure access still exist through the active `StdbConnection`, but the primary Bevy-facing event flow is table/message based.

The project was heavily inspired by [`bevy_spacetimedb`](https://docs.rs/bevy_spacetimedb/), but takes a different approach to plugin structure, connection lifecycle handling, reconnect behavior, and subscription restoration.
