pub mod controller;
pub mod toy;

use crate::sessions::port::SessionStore;
use serde::{Deserialize, Serialize};
use socketioxide::{
    extract::{SocketRef, TryData},
    SocketIo,
};
use tracing::{debug, warn};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum Role {
    Toy,
    Controller,
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Auth {
    pub role: Role,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ConnectError {
    Unauthorized,
}

#[derive(Serialize)]
struct ConnectErrorResponse {
    reason: ConnectError,
}

impl ConnectErrorResponse {
    fn with_reason(reason: ConnectError) -> Self {
        ConnectErrorResponse { reason }
    }
}

pub fn on_connect<T>(socket: SocketRef, io: SocketIo, TryData(auth): TryData<Auth>)
where
    T: SessionStore + 'static,
{
    debug!("Client connected: {:?}", socket.id);

    let role = match auth {
        Ok(data) => data.role,
        Err(error) => {
            warn!(%error, "Client provided invalid auth data");
            socket
                .emit(
                    "connect_error",
                    ConnectErrorResponse::with_reason(ConnectError::Unauthorized),
                )
                .ok();
            socket.disconnect().ok();
            return;
        }
    };

    match role {
        Role::Toy => toy::on_connect::<T>(socket),
        Role::Controller => controller::on_connect::<T>(socket, io),
    };
}
