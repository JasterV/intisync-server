mod handlers;
mod messages;

use crate::{sessions::port::SessionStore, socket::adapters::local::ClientSocketImpl};
use handlers::{on_disconnect, on_start_session};
use socketioxide::extract::{AckSender, SocketRef, State};
use tracing::error;

pub fn on_connect<T>(socket: SocketRef)
where
    T: SessionStore + 'static,
{
    socket.on(
        "start_session",
        |socket: SocketRef, ack: AckSender, sessions: State<T>| async move {
            if let Err(error) =
                ack.send(on_start_session(ClientSocketImpl::from(socket), sessions.0).await)
            {
                error!(%error, "Failed to send acknowledgment to client.");
            }
        },
    );
    socket.on_disconnect(|socket: SocketRef, sessions: State<T>| async move {
        on_disconnect(ClientSocketImpl::from(socket), sessions.0).await
    });
}
