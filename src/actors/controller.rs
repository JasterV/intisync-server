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
    use futures_util::FutureExt;
    use mockall::predicate::eq;
    use socketioxide::extract::Data;
    use std::time::Duration;
    use test_context::{test_context, AsyncTestContext};
    use uuid::Uuid;

    struct Context {
        client_socket: MockClientSocket,
        session_store: MockSessionStore,
        global_socket: MockGlobalSocket,
    }

    impl AsyncTestContext for Context {
        async fn setup() -> Self {
            Self {
                client_socket: MockClientSocket::new(),
                global_socket: MockGlobalSocket::new(),
                session_store: MockSessionStore::new(),
            }
        }
    }

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn can_not_join_a_session_if_already_in_a_session(mut ctx: Context) {
        let join_request = Data(JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        });
        let config = Config::load();

        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(Some(Uuid::nil()));

        let result = on_join_session(
            ctx.client_socket,
            ctx.global_socket,
            join_request,
            &ctx.session_store,
            &config,
        )
        .await;

        assert_eq!(
            result,
            JoinSessionResponse::with_err(JoinSessionErrorKind::AlreadyInASession)
        );
    }

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn can_not_join_non_existent_session(mut ctx: Context) {
        let join_request = Data(JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        });
        let config = Config::load();

        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        ctx.session_store
            .expect_session_state()
            .times(1)
            .returning(|_| async { Ok(None) }.boxed());

        let result = on_join_session(
            ctx.client_socket,
            ctx.global_socket,
            join_request,
            &ctx.session_store,
            &config,
        )
        .await;

        assert_eq!(
            result,
            JoinSessionResponse::with_err(JoinSessionErrorKind::SessionNotFound)
        );
    }

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn can_not_join_a_full_session(mut ctx: Context) {
        let join_request = Data(JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        });
        let config = Config::load();

        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        ctx.session_store
            .expect_session_state()
            .times(1)
            .returning(|_| async { Ok(Some(SessionState::InProgress)) }.boxed());

        let result = on_join_session(
            ctx.client_socket,
            ctx.global_socket,
            join_request,
            &ctx.session_store,
            &config,
        )
        .await;

        assert_eq!(
            result,
            JoinSessionResponse::with_err(JoinSessionErrorKind::SessionFull)
        );
    }

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn can_join_a_session_if_hub_accepts(mut ctx: Context) {
        let join_request = JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        };
        let config = Config::load();

        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        ctx.session_store
            .expect_session_state()
            .times(1)
            .returning(|_| async { Ok(Some(SessionState::WaitingForController)) }.boxed());

        ctx.global_socket
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
            .returning(|_, _, _, _| async { Ok(JoinSessionPermissionResponse::Accept) }.boxed());

        ctx.client_socket
            .expect_join()
            .times(1)
            .return_const(Ok(()));

        ctx.session_store
            .expect_update_session_state()
            .times(1)
            .with(eq(join_request.session_id), eq(SessionState::InProgress))
            .returning(|_, _| async { Ok(()) }.boxed());

        ctx.client_socket
            .expect_emit_to_room()
            .times(1)
            .with(
                eq(join_request.session_id.to_string()),
                eq("controller_joined".to_string()),
                eq(()),
            )
            .return_const(Ok(()));

        ctx.client_socket
            .expect_store_value()
            .times(1)
            .with(eq(join_request.session_id))
            .return_const(());

        let result = on_join_session(
            ctx.client_socket,
            ctx.global_socket,
            Data(join_request),
            &ctx.session_store,
            &config,
        )
        .await;

        assert_eq!(result, JoinSessionResponse::Ok {});
    }

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn do_not_join_a_session_if_hub_rejects(mut ctx: Context) {
        let join_request = JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        };
        let config = Config::load();

        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        ctx.session_store
            .expect_session_state()
            .times(1)
            .returning(|_| async { Ok(Some(SessionState::WaitingForController)) }.boxed());

        ctx.global_socket
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
            .returning(|_, _, _, _| async { Ok(JoinSessionPermissionResponse::Reject) }.boxed());

        ctx.client_socket.expect_join().never();
        ctx.session_store.expect_update_session_state().never();
        ctx.client_socket.expect_emit_to_room::<()>().never();
        ctx.client_socket.expect_store_value().never();

        let result = on_join_session(
            ctx.client_socket,
            ctx.global_socket,
            Data(join_request),
            &ctx.session_store,
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

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn return_hub_timeout_error_if_hub_returns_an_error(mut ctx: Context) {
        let join_request = JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        };
        let config = Config::load();

        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        ctx.session_store
            .expect_session_state()
            .times(1)
            .returning(|_| async { Ok(Some(SessionState::WaitingForController)) }.boxed());

        ctx.global_socket
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
            .returning(|_, _, _, _| async { Err(DummyMockError) }.boxed());

        ctx.client_socket.expect_join().never();
        ctx.session_store.expect_update_session_state().never();
        ctx.client_socket.expect_emit_to_room::<()>().never();
        ctx.client_socket.expect_store_value().never();

        let result = on_join_session(
            ctx.client_socket,
            ctx.global_socket,
            Data(join_request),
            &ctx.session_store,
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
