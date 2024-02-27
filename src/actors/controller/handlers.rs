use super::messages::*;
use crate::{
    configuration::Config,
    sessions::port::{SessionState, SessionStore},
    socket::port::{ClientSocket, GlobalSocket},
};
use socketioxide::extract::Data;
use std::time::Duration;
use tracing::{debug, error, warn};
use uuid::Uuid;

pub async fn on_join_session<T, S, G>(
    socket: S,
    global_socket: G,
    Data(request): Data<JoinSessionRequest>,
    sessions: &T,
    config: &Config,
) -> JoinSessionResponse
where
    T: SessionStore,
    S: ClientSocket<StoreItem = Uuid>,
    G: GlobalSocket,
{
    debug!("Received join_session command");

    let session_id = request.session_id;

    if socket.get_stored_value().is_some() {
        return JoinSessionResponse::with_err(JoinSessionErrorKind::AlreadyInASession);
    }

    match sessions.session_state(session_id).await {
        Ok(Some(SessionState::InProgress)) => {
            return JoinSessionResponse::with_err(JoinSessionErrorKind::SessionFull);
        }
        Ok(Some(SessionState::WaitingForController)) => (),
        Ok(None) => {
            return JoinSessionResponse::with_err(JoinSessionErrorKind::SessionNotFound);
        }
        Err(error) => {
            error!(%error, "Failed to check session state");
            return JoinSessionResponse::with_err(JoinSessionErrorKind::ServerError);
        }
    };

    let response = global_socket
        .emit_to_room_with_ack(
            session_id.into(),
            "join_request".into(),
            JoinSessionPermissionRequest {
                message: request.message,
            },
            Duration::from_secs(config.controller.session_join_request_timeout),
        )
        .await;

    match response {
        Ok(JoinSessionPermissionResponse::Accept) => (),
        Ok(JoinSessionPermissionResponse::Reject) => {
            return JoinSessionResponse::with_err(JoinSessionErrorKind::Rejected);
        }
        Err(error) => {
            error!(%error, "Failed to ask client if controller can join the session");
            return JoinSessionResponse::with_err(JoinSessionErrorKind::HubResponseTimeout);
        }
    };

    if let Err(error) = sessions
        .update_session_state(session_id, SessionState::InProgress)
        .await
    {
        error!(%error, "Failed to update session state");
        return JoinSessionResponse::with_err(JoinSessionErrorKind::ServerError);
    }

    if let Err(error) = socket.join(session_id.into()) {
        error!(%error, "Controller failed to join session");
        return JoinSessionResponse::with_err(JoinSessionErrorKind::ServerError);
    }

    if let Err(error) = socket.emit_to_room(session_id.into(), "controller_joined".into(), ()) {
        error!(%error, "Controller failed to send join confirmation to hub");
        return JoinSessionResponse::with_err(JoinSessionErrorKind::ServerError);
    }

    socket.store_value(session_id);
    JoinSessionResponse::Ok {}
}

pub fn on_vibrate_command<S>(socket: S, Data(cmd): Data<VibrateCmd>)
where
    S: ClientSocket<StoreItem = Uuid>,
{
    debug!("Received vibrate command");
    let Some(session_id) = socket.get_stored_value() else {
        warn!("Client sent vibrate command without being in a session");
        socket
            .emit(
                "error".into(),
                ControllerErrorMsg::new(
                    ControllerErrorKind::Permissions,
                    "Can't send a vibrate command if not in a session",
                ),
            )
            .ok();
        socket.disconnect();
        return;
    };

    if let Err(error) = socket.emit_to_room(session_id.into(), "vibrate".into(), cmd) {
        error!(%error, "Failed to emit vibrate command");
        socket
            .emit(
                "error".into(),
                ControllerErrorMsg::new(
                    ControllerErrorKind::VibrateCmdSendError,
                    "Failed to send vibration command",
                ),
            )
            .ok();
    }
}

pub async fn on_disconnect<T, S>(socket: S, sessions: &T)
where
    T: SessionStore,
    S: ClientSocket<StoreItem = Uuid>,
{
    debug!("Controller disconnected");

    let Some(session_id) = socket.get_stored_value() else {
        return;
    };

    if let Err(error) = socket.emit_to_room(session_id.into(), "controller_disconnected".into(), ())
    {
        error!(%error, "Failed to send controller_disconnected event");
    }

    if let Err(error) = sessions
        .update_session_state(session_id, SessionState::WaitingForController)
        .await
    {
        error!(%error, "Failed to update session state");
    }
}
