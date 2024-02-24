mod handlers;
mod messages;

use crate::{
    configuration::Config,
    sessions::port::SessionStore,
    socket::adapters::local::{ClientSocketImpl, GlobalSocketImpl},
};
use handlers::{on_disconnect, on_join_session, on_vibrate_command};
pub use messages::*;
use socketioxide::{
    extract::{AckSender, Data, SocketRef, State},
    SocketIo,
};
use tracing::error;

pub fn on_connect<T>(socket: SocketRef, io: SocketIo)
where
    T: SessionStore + 'static,
{
    socket.on(
        "join_session",
        |socket: SocketRef,
         data: Data<JoinSessionRequest>,
         ack: AckSender,
         sessions: State<T>,
         config: State<Config>| async move {
            if let Err(error) = ack.send(
                on_join_session(
                    ClientSocketImpl::from(socket),
                    GlobalSocketImpl::from(io),
                    data,
                    sessions.0,
                    config.0,
                )
                .await,
            ) {
                error!(%error, "Failed to send acknowledgment to client");
            }
        },
    );
    socket.on("vibrate", |socket: SocketRef, data: Data<VibrateCmd>| {
        on_vibrate_command(ClientSocketImpl::from(socket), data)
    });
    socket.on_disconnect(|socket: SocketRef, sessions: State<T>| async move {
        on_disconnect(ClientSocketImpl::from(socket), sessions.0).await
    });
}
