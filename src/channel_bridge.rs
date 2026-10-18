//! Channel-backed message delivery for Bevy.
//!
//! Registers per-type channels and forwards messages from those
//! channels into Bevy [`Messages<T>`](bevy_ecs::prelude::Messages), such as SpacetimeDB table events
//! or connection lifecycle messages.
use crate::set::StdbSet;
use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::{IntoScheduleConfigs, Message, Messages, Mut, Resource, World};
use crossbeam_channel::unbounded;
use std::any::{Any, TypeId, type_name};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Stores one registered message channel.
struct ChannelEntry {
    /// The registered message type.
    type_id: TypeId,
    clear: Box<dyn Fn(&mut World) + Send + Sync>,
    /// A type-erased function that drains a channel into `Messages<T>`.
    drain: Box<dyn Fn(&mut World) + Send + Sync>,
    /// The sender for this message type.
    sender: Box<dyn Any + Send + Sync>,
}

/// Registry of per-type message channels that bridge crossbeam senders into
/// Bevy [`Messages<T>`](bevy_ecs::prelude::Messages).
///
/// Internal channels (connection lifecycle, table events, subscriptions) and
/// consumer channels registered with
/// [`StdbPlugin::add_channel_message`](crate::prelude::StdbPlugin::add_channel_message)
/// all live here. Consumers obtain a sender for their message type with [`Self::sender`]
/// and move it into a callback or task running off the Bevy schedule (a
/// SpacetimeDB reducer/procedure `_then` handler, an HTTP response handler, an
/// `IoTaskPool` task, ...) to forward a value into Bevy; read it back with
/// `MessageReader<T>`.
#[derive(Resource, Default)]
pub struct StdbChannels {
    channels: Vec<ChannelEntry>,
    generation: Arc<AtomicU64>,
}

/// A sender bound to the connection generation that created its callback.
pub struct Sender<T> {
    inner: crossbeam_channel::Sender<(u64, T)>,
    generation: Arc<AtomicU64>,
    captured: u64,
}
impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            generation: self.generation.clone(),
            captured: self.captured,
        }
    }
}
impl<T> Sender<T> {
    pub(crate) fn current(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            generation: self.generation.clone(),
            captured: self.generation.load(Ordering::SeqCst),
        }
    }
    /// Sends a value tagged with the connection generation captured by this sender.
    pub fn send(&self, value: T) -> Result<(), crossbeam_channel::SendError<T>> {
        self.inner
            .send((self.captured, value))
            .map_err(|e| crossbeam_channel::SendError(e.0.1))
    }
}

pub(crate) fn invalidate(world: &mut World) {
    world.resource_scope(|world, channels: bevy_ecs::prelude::Mut<StdbChannels>| {
        channels.generation.fetch_add(1, Ordering::SeqCst);
        for channel in &channels.channels {
            (channel.clear)(world);
        }
    });
}

impl StdbChannels {
    /// Returns a clone of the registered [`Sender<T>`] for message type `T`.
    ///
    /// The sender captures the current connection generation. Disconnecting
    /// invalidates it and its clones; obtain a new sender for subsequent work.
    ///
    /// Move the returned sender into a callback or task and forward with
    /// `tx.send(message)`.
    ///
    /// # Panics
    ///
    /// Panics if no channel for `T` was registered via
    /// [`StdbPlugin::add_channel_message`](crate::prelude::StdbPlugin::add_channel_message).
    pub fn sender<T: Message>(&self) -> Sender<T> {
        self.channels
            .iter()
            .find(|entry| entry.type_id == TypeId::of::<T>())
            .and_then(|entry| entry.sender.downcast_ref::<Sender<T>>())
            .unwrap_or_else(|| panic!("unregistered channel for `{}`", type_name::<T>()))
            .current()
    }
}

pub(crate) struct ChannelBridgePlugin {
    pub(crate) channel_registrations: Vec<fn(&mut App)>,
}
impl Plugin for ChannelBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StdbChannels>();
        app.add_systems(PreUpdate, drain_channels.in_set(StdbSet::Flush));

        for register in &self.channel_registrations {
            register(app);
        }
    }
}

/// Drains all registered channels once per frame.
fn drain_channels(world: &mut World) {
    world.resource_scope(|world, registry: Mut<StdbChannels>| {
        for entry in &registry.channels {
            (entry.drain)(world);
        }
    });
}

/// Registers a channel for message type `T`.
///
/// # Panics
///
/// Panics if [`StdbChannels`] has not been initialized or if the channel for
/// `T` has already been registered.
pub(crate) fn register_channel<T: Message>(app: &mut App) {
    assert!(
        !app.world()
            .resource::<StdbChannels>()
            .channels
            .iter()
            .any(|entry| entry.type_id == TypeId::of::<T>()),
        "attempted to register channel for message type `{}` more than once",
        type_name::<T>(),
    );

    let (tx, rx) = unbounded::<(u64, T)>();
    let generation = app.world().resource::<StdbChannels>().generation.clone();
    let tx = Sender {
        inner: tx,
        captured: generation.load(Ordering::SeqCst),
        generation: generation.clone(),
    };
    app.add_message::<T>();

    app.world_mut()
        .resource_mut::<StdbChannels>()
        .channels
        .push(ChannelEntry {
            type_id: TypeId::of::<T>(),
            clear: Box::new(|world| world.resource_mut::<Messages<T>>().clear()),
            drain: Box::new(move |world: &mut World| {
                world.resource_mut::<Messages<T>>().write_batch(
                    rx.try_iter()
                        .filter(|(epoch, _)| *epoch == generation.load(Ordering::SeqCst))
                        .map(|(_, value)| value),
                );
            }),
            sender: Box::new(tx),
        });
}

/// Returns the registered `Sender<T>` from the world.
///
/// # Panics
///
/// Panics if [`StdbChannels`] has not been initialized, or if the channel for
/// `T` has not been registered.
pub(crate) fn channel_sender<T: Message>(world: &World) -> Sender<T> {
    world.resource::<StdbChannels>().sender::<T>()
}

#[cfg(test)]
mod generation_tests {
    use super::*;
    #[derive(Message, Clone)]
    struct Callback(u8);

    #[test]
    fn reconnect_discards_buffered_and_late_callbacks_from_old_connections() {
        let mut app = App::new();
        app.init_resource::<StdbChannels>();
        register_channel::<Callback>(&mut app);
        let old = channel_sender::<Callback>(app.world());
        old.send(Callback(1)).unwrap();
        drain_channels(app.world_mut());
        old.send(Callback(2)).unwrap(); // Queued before disconnect, not yet drained.
        let old_clone = old.clone();
        invalidate(app.world_mut());
        old.send(Callback(4)).unwrap(); // Late callback from the previous connection.
        old_clone.send(Callback(5)).unwrap();
        channel_sender::<Callback>(app.world())
            .send(Callback(3))
            .unwrap();
        drain_channels(app.world_mut());
        let delivered: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<Callback>>()
            .drain()
            .map(|c| c.0)
            .collect();
        assert_eq!(delivered, vec![3]);
    }
}
