use crate::socket::port::{ClientSocket, GlobalSocket, MessageWithAck};
use serde::Serialize;
use socketioxide::{
    adapter::LocalAdapter, extract::SocketRef, AckError, BroadcastError, SendError, SocketIo,
};
use std::{convert::Infallible, time::Duration};
use uuid::Uuid;

pub struct ClientSocketImpl(SocketRef<LocalAdapter>);

impl From<SocketRef> for ClientSocketImpl {
    fn from(value: SocketRef) -> Self {
        Self(value)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum EmitWithAckError {
    #[error("Failed to receive acknowledgment: '{0}'")]
    AckError(#[from] AckError),
}

#[derive(thiserror::Error, Debug)]
pub enum EmitError {
    #[error("Failed to broadcast message: '{0}'")]
    BroadcastError(#[from] BroadcastError),
    #[error("Failed to send message: '{0}'")]
    SendError(#[from] SendError),
}

impl ClientSocket for ClientSocketImpl {
    type Error = Infallible;
    type EmitError = EmitError;
    type StoreItem = Uuid;

    fn join(&self, room: String) -> Result<(), Self::Error> {
        self.0.join(room)?;
        Ok(())
    }

    fn leave(&self, room: String) -> Result<(), Self::Error> {
        self.0.leave(room)?;
        Ok(())
    }

    fn emit_to_room<T>(&self, room: String, event: String, data: T) -> Result<(), Self::EmitError>
    where
        T: Serialize + Send,
    {
        self.0.to(room).emit(event, data)?;
        Ok(())
    }

    fn emit<T>(&self, event: String, data: T) -> Result<(), Self::EmitError>
    where
        T: Serialize + Send,
    {
        self.0.emit(event, data)?;
        Ok(())
    }

    fn disconnect(self) {
        let _ = self.0.disconnect();
    }

    fn get_stored_value(&self) -> Option<Self::StoreItem> {
        self.0
            .extensions
            .get::<Self::StoreItem>()
            .map(|value| value.to_owned())
    }

    fn remove_value(&self) {
        self.0.extensions.remove::<Self::StoreItem>();
    }

    fn store_value(&self, value: Self::StoreItem) {
        self.0.extensions.insert(value);
    }
}

pub struct GlobalSocketImpl(SocketIo<LocalAdapter>);

impl From<SocketIo<LocalAdapter>> for GlobalSocketImpl {
    fn from(value: SocketIo<LocalAdapter>) -> Self {
        Self(value)
    }
}

impl GlobalSocket for GlobalSocketImpl {
    type EmitWithAckError = EmitWithAckError;

    async fn emit_to_room_with_ack<T>(
        &self,
        room: String,
        event: String,
        value: T,
        timeout: Duration,
    ) -> Result<T::Ack, Self::EmitWithAckError>
    where
        T: MessageWithAck,
    {
        let response = self
            .0
            .to(room)
            .timeout(timeout)
            .emit_with_ack::<T::Ack>(event, value)
            .unwrap()
            .await?;

        Ok(response.data)
    }
}
