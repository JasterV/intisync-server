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

#[cfg(test)]
mod tests {
    use super::on_join_session;
    use crate::{
        actors::controller::{
            JoinSessionErrorKind, JoinSessionPermissionRequest, JoinSessionPermissionResponse,
            JoinSessionRequest, JoinSessionResponse,
        },
        configuration::Config,
        sessions::port::{MockSessionStore, SessionState},
        socket::port::{DummyMockError, MockClientSocket, MockGlobalSocket},
    };
    use mockall::predicate::eq;
    use socketioxide::extract::Data;
    use std::time::Duration;
    use uuid::Uuid;

    #[tokio::test]
    async fn can_not_join_a_session_if_already_in_a_session() {
        let mut client_socket = MockClientSocket::new();
        let global_socket = MockGlobalSocket::new();
        let sessions = MockSessionStore::new();
        let join_request = Data(JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        });
        let config = Config::load();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(Some(Uuid::nil()));

        let result = on_join_session(
            client_socket,
            global_socket,
            join_request,
            &sessions,
            &config,
        )
        .await;

        assert_eq!(
            result,
            JoinSessionResponse::with_err(JoinSessionErrorKind::AlreadyInASession)
        );
    }

    #[tokio::test]
    async fn can_not_join_non_existent_session() {
        let mut client_socket = MockClientSocket::new();
        let global_socket = MockGlobalSocket::new();
        let mut sessions = MockSessionStore::new();
        let join_request = Data(JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        });
        let config = Config::load();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        sessions
            .expect_session_state()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));

        let result = on_join_session(
            client_socket,
            global_socket,
            join_request,
            &sessions,
            &config,
        )
        .await;

        assert_eq!(
            result,
            JoinSessionResponse::with_err(JoinSessionErrorKind::SessionNotFound)
        );
    }

    #[tokio::test]
    async fn can_not_join_a_full_session() {
        let mut client_socket = MockClientSocket::new();
        let global_socket = MockGlobalSocket::new();
        let mut sessions = MockSessionStore::new();
        let join_request = Data(JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        });
        let config = Config::load();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        sessions
            .expect_session_state()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(SessionState::InProgress)) }));

        let result = on_join_session(
            client_socket,
            global_socket,
            join_request,
            &sessions,
            &config,
        )
        .await;

        assert_eq!(
            result,
            JoinSessionResponse::with_err(JoinSessionErrorKind::SessionFull)
        );
    }

    #[tokio::test]
    async fn can_join_a_session_if_hub_accepts() {
        let mut client_socket = MockClientSocket::new();
        let mut global_socket = MockGlobalSocket::new();
        let mut sessions = MockSessionStore::new();
        let join_request = JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        };
        let config = Config::load();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        sessions
            .expect_session_state()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(SessionState::WaitingForController)) }));

        global_socket
            .expect_emit_to_room_with_ack()
            .times(1)
            .with(
                eq(join_request.session_id.to_string()),
                eq("join_request".to_string()),
                eq(JoinSessionPermissionRequest {
                    message: join_request.message.clone(),
                }),
                eq(Duration::from_secs(
                    config.controller.session_join_request_timeout,
                )),
            )
            .returning(|_, _, _, _| Box::pin(async { Ok(JoinSessionPermissionResponse::Accept) }));

        client_socket.expect_join().times(1).return_const(Ok(()));

        sessions
            .expect_update_session_state()
            .times(1)
            .with(eq(join_request.session_id), eq(SessionState::InProgress))
            .returning(|_, _| Box::pin(async { Ok(()) }));

        client_socket
            .expect_emit_to_room()
            .times(1)
            .with(
                eq(join_request.session_id.to_string()),
                eq("controller_joined".to_string()),
                eq(()),
            )
            .return_const(Ok(()));

        client_socket
            .expect_store_value()
            .times(1)
            .with(eq(join_request.session_id))
            .return_const(());

        let result = on_join_session(
            client_socket,
            global_socket,
            Data(join_request),
            &sessions,
            &config,
        )
        .await;

        assert_eq!(result, JoinSessionResponse::Ok {});
    }

    #[tokio::test]
    async fn do_not_join_a_session_if_hub_rejects() {
        let mut client_socket = MockClientSocket::new();
        let mut global_socket = MockGlobalSocket::new();
        let mut sessions = MockSessionStore::new();
        let join_request = JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        };
        let config = Config::load();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        sessions
            .expect_session_state()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(SessionState::WaitingForController)) }));

        global_socket
            .expect_emit_to_room_with_ack()
            .times(1)
            .with(
                eq(join_request.session_id.to_string()),
                eq("join_request".to_string()),
                eq(JoinSessionPermissionRequest {
                    message: join_request.message.clone(),
                }),
                eq(Duration::from_secs(
                    config.controller.session_join_request_timeout,
                )),
            )
            .returning(|_, _, _, _| Box::pin(async { Ok(JoinSessionPermissionResponse::Reject) }));

        client_socket.expect_join().never();
        sessions.expect_update_session_state().never();
        client_socket.expect_emit_to_room::<()>().never();
        client_socket.expect_store_value().never();

        let result = on_join_session(
            client_socket,
            global_socket,
            Data(join_request),
            &sessions,
            &config,
        )
        .await;

        assert_eq!(
            result,
            JoinSessionResponse::Error {
                kind: JoinSessionErrorKind::Rejected
            }
        );
    }

    #[tokio::test]
    async fn return_hub_timeout_error_if_hub_returns_an_error() {
        let mut client_socket = MockClientSocket::new();
        let mut global_socket = MockGlobalSocket::new();
        let mut sessions = MockSessionStore::new();
        let join_request = JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        };
        let config = Config::load();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        sessions
            .expect_session_state()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(SessionState::WaitingForController)) }));

        global_socket
            .expect_emit_to_room_with_ack()
            .times(1)
            .with(
                eq(join_request.session_id.to_string()),
                eq("join_request".to_string()),
                eq(JoinSessionPermissionRequest {
                    message: join_request.message.clone(),
                }),
                eq(Duration::from_secs(
                    config.controller.session_join_request_timeout,
                )),
            )
            .returning(|_, _, _, _| Box::pin(async { Err(DummyMockError) }));

        client_socket.expect_join().never();
        sessions.expect_update_session_state().never();
        client_socket.expect_emit_to_room::<()>().never();
        client_socket.expect_store_value().never();

        let result = on_join_session(
            client_socket,
            global_socket,
            Data(join_request),
            &sessions,
            &config,
        )
        .await;

        assert_eq!(
            result,
            JoinSessionResponse::Error {
                kind: JoinSessionErrorKind::HubResponseTimeout
            }
        );
    }
}
