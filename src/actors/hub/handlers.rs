use super::messages::*;
use crate::{sessions::port::SessionStore, socket::port::ClientSocket};
use tracing::{debug, error};
use uuid::Uuid;

pub async fn on_start_session<T, S>(socket: S, sessions: &T) -> StartSessionResponse
where
    S: ClientSocket<StoreItem = Uuid>,
    T: SessionStore,
{
    debug!("Received start_session command");

    if socket.get_stored_value().is_some() {
        return StartSessionResponse::error(StartSessionError::AlreadyInASession);
    }

    let session_id = match sessions.create_session().await {
        Ok(session_id) => session_id,
        Err(error) => {
            error!(%error, "Failed to create session on session store");
            return StartSessionResponse::error(StartSessionError::ServerError);
        }
    };

    if let Err(error) = socket.join(session_id.into()) {
        error!(%error, "Socket failed to join session");
        return StartSessionResponse::error(StartSessionError::ServerError);
    }

    socket.store_value(session_id);

    StartSessionResponse::Ok { session_id }
}

pub async fn on_disconnect<T, S>(socket: S, sessions: &T)
where
    T: SessionStore,
    S: ClientSocket<StoreItem = Uuid>,
{
    let Some(session_id) = socket.get_stored_value() else {
        return;
    };

    if let Err(error) = socket.emit_to_room(session_id.into(), "session_finished".into(), ()) {
        error!(%error, "Failed to send session_finished event");
    }

    socket.remove_value();

    if let Err(error) = sessions.delete_session(session_id).await {
        error!(%error, "Failed to delete session");
    }
}
