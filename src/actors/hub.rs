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

#[cfg(test)]
mod tests {
    use super::{on_disconnect, on_start_session};
    use crate::{
        actors::hub::messages::{StartSessionError, StartSessionResponse},
        sessions::port::MockSessionStore,
        socket::port::MockClientSocket,
    };
    use mockall::predicate::eq;
    use test_context::{test_context, AsyncTestContext};
    use uuid::Uuid;

    struct Context {
        client_socket: MockClientSocket,
        session_store: MockSessionStore,
    }

    impl AsyncTestContext for Context {
        async fn setup() -> Self {
            Self {
                client_socket: MockClientSocket::new(),
                session_store: MockSessionStore::new(),
            }
        }
    }

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn creates_a_session_on_start_session_command(mut ctx: Context) {
        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        ctx.session_store
            .expect_create_session()
            .times(1)
            .returning(|| Box::pin(async move { Ok(Uuid::nil()) }));

        ctx.client_socket
            .expect_join()
            .with(eq(Uuid::nil().to_string()))
            .times(1)
            .return_const(Ok(()));

        ctx.client_socket
            .expect_store_value()
            .with(eq(Uuid::nil()))
            .times(1)
            .return_const(());

        let result = on_start_session(ctx.client_socket, &ctx.session_store).await;

        assert!(matches!(result, StartSessionResponse::Ok { session_id: _ }));
    }

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn does_not_create_a_session_if_already_in_a_session(mut ctx: Context) {
        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(Some(Uuid::nil()));

        ctx.session_store.expect_create_session().never();
        ctx.client_socket.expect_join().never();
        ctx.client_socket.expect_store_value().never();

        let result = on_start_session(ctx.client_socket, &ctx.session_store).await;

        assert_eq!(
            result,
            StartSessionResponse::error(StartSessionError::AlreadyInASession)
        );
    }

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn deletes_session_and_sends_message_on_disconnect_if_in_a_session(mut ctx: Context) {
        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(Some(Uuid::nil()));

        ctx.client_socket
            .expect_emit_to_room()
            .times(1)
            .with(
                eq(Uuid::nil().to_string()),
                eq("session_finished".to_string()),
                eq(()),
            )
            .return_const(Ok(()));

        ctx.client_socket
            .expect_remove_value()
            .times(1)
            .return_const(());

        ctx.session_store
            .expect_delete_session()
            .times(1)
            .with(eq(Uuid::nil()))
            .returning(|_| Box::pin(async { Ok(()) }));

        on_disconnect(ctx.client_socket, &ctx.session_store).await;
    }

    #[test_context(Context, skip_teardown)]
    #[tokio::test]
    async fn doesnt_do_anything_if_not_in_a_session_when_disconnect(mut ctx: Context) {
        ctx.client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        ctx.client_socket.expect_emit_to_room::<()>().never();
        ctx.client_socket.expect_remove_value().never();
        ctx.session_store.expect_delete_session().never();

        on_disconnect(ctx.client_socket, &ctx.session_store).await;
    }
}
