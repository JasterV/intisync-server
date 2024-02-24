#[cfg(test)]
pub use mocks::*;
use serde::{de::DeserializeOwned, Serialize};
#[cfg(test)]
use std::convert::Infallible;
use std::{future::Future, time::Duration};
#[cfg(test)]
use uuid::Uuid;

pub trait MessageWithAck: Serialize + Send + 'static {
    type Ack: DeserializeOwned + Send;
}

#[cfg_attr(test, mockall::automock(type Error = Infallible; type EmitWithAckError = Infallible; type EmitError = Infallible; type StoreItem = Uuid;))]
pub trait ClientSocket: Send {
    type Error: std::error::Error + Send;
    type EmitError: std::error::Error + Send;
    type StoreItem: Send + Sync + Copy + 'static;

    fn disconnect(self);

    fn join(&self, room: String) -> Result<(), Self::Error>;

    fn leave(&self, room: String) -> Result<(), Self::Error>;

    fn emit_to_room<T>(&self, room: String, event: String, value: T) -> Result<(), Self::EmitError>
    where
        T: Serialize + Send + 'static;

    fn emit<T>(&self, event: String, value: T) -> Result<(), Self::EmitError>
    where
        T: Serialize + Send + 'static;

    fn get_stored_value(&self) -> Option<Self::StoreItem>;

    fn remove_value(&self);

    fn store_value(&self, value: Self::StoreItem);
}

#[cfg_attr(test, mockall::automock(type EmitWithAckError = DummyMockError; type EmitError = Infallible;))]
pub trait GlobalSocket: Send {
    type EmitWithAckError: std::error::Error + Send;

    fn emit_to_room_with_ack<T>(
        &self,
        room: String,
        event: String,
        value: T,
        timeout: Duration,
    ) -> impl Future<Output = Result<T::Ack, Self::EmitWithAckError>> + Send
    where
        T: MessageWithAck;
}

#[cfg(test)]
mod mocks {
    #[derive(thiserror::Error, Debug)]
    #[error("Dummy error")]
    pub struct DummyMockError;
}
