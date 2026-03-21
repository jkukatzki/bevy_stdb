//! Convenience aliases for reading `bevy_stdb` messages from Bevy systems.
//!
//! This module groups the `MessageReader` aliases used for connection lifecycle
//! messages and table-driven update messages so system signatures can stay concise.

use crate::message::{
    DeleteMessage, InsertMessage, InsertUpdateMessage, StdbConnectedMessage,
    StdbConnectionErrorMessage, StdbDisconnectedMessage, UpdateMessage,
};
use bevy_ecs::prelude::MessageReader;

/// A [`MessageReader`] for [`InsertMessage<T>`] events.
pub type ReadInsertMessage<'w, 's, T> = MessageReader<'w, 's, InsertMessage<T>>;

/// A [`MessageReader`] for [`UpdateMessage<T>`] events.
pub type ReadUpdateMessage<'w, 's, T> = MessageReader<'w, 's, UpdateMessage<T>>;

/// A [`MessageReader`] for [`DeleteMessage<T>`] events.
pub type ReadDeleteMessage<'w, 's, T> = MessageReader<'w, 's, DeleteMessage<T>>;

/// A [`MessageReader`] for combined [`InsertUpdateMessage<T>`] events.
pub type ReadInsertUpdateMessage<'w, 's, T> = MessageReader<'w, 's, InsertUpdateMessage<T>>;

/// A [`MessageReader`] for [`StdbConnectedMessage`] events.
pub type ReadStdbConnectedMessage<'w, 's> = MessageReader<'w, 's, StdbConnectedMessage>;

/// A [`MessageReader`] for [`StdbDisconnectedMessage`] events.
pub type ReadStdbDisconnectedMessage<'w, 's> = MessageReader<'w, 's, StdbDisconnectedMessage>;

/// A [`MessageReader`] for [`StdbConnectionErrorMessage`] events.
pub type ReadStdbConnectionErrorMessage<'w, 's> = MessageReader<'w, 's, StdbConnectionErrorMessage>;
