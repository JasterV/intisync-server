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

#[cfg(test)]
mod tests {
    use mockall::predicate::eq;
    use uuid::Uuid;

    use super::{on_disconnect, on_start_session};
    use crate::{
        actors::hub::messages::{StartSessionError, StartSessionResponse},
        sessions::port::MockSessionStore,
        socket::port::MockClientSocket,
    };

    #[tokio::test]
    async fn creates_a_session_on_start_session_command() {
        let mut client_socket = MockClientSocket::new();
        let mut sessions = MockSessionStore::new();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        sessions
            .expect_create_session()
            .times(1)
            .returning(|| Box::pin(async move { Ok(Uuid::nil()) }));

        client_socket
            .expect_join()
            .with(eq(Uuid::nil().to_string()))
            .times(1)
            .return_const(Ok(()));

        client_socket
            .expect_store_value()
            .with(eq(Uuid::nil()))
            .times(1)
            .return_const(());

        let result = on_start_session(client_socket, &sessions).await;

        assert!(matches!(result, StartSessionResponse::Ok { session_id: _ }));
    }

    #[tokio::test]
    async fn does_not_create_a_session_if_already_in_a_session() {
        let mut client_socket = MockClientSocket::new();
        let mut sessions = MockSessionStore::new();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(Some(Uuid::nil()));

        sessions.expect_create_session().never();
        client_socket.expect_join().never();
        client_socket.expect_store_value().never();

        let result = on_start_session(client_socket, &sessions).await;

        assert_eq!(
            result,
            StartSessionResponse::error(StartSessionError::AlreadyInASession)
        );
    }

    #[tokio::test]
    async fn deletes_session_and_sends_message_on_disconnect_if_in_a_session() {
        let mut client_socket = MockClientSocket::new();
        let mut sessions = MockSessionStore::new();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(Some(Uuid::nil()));

        client_socket
            .expect_emit_to_room()
            .times(1)
            .with(
                eq(Uuid::nil().to_string()),
                eq("session_finished".to_string()),
                eq(()),
            )
            .return_const(Ok(()));

        client_socket
            .expect_remove_value()
            .times(1)
            .return_const(());

        sessions
            .expect_delete_session()
            .times(1)
            .with(eq(Uuid::nil()))
            .returning(|_| Box::pin(async { Ok(()) }));

        on_disconnect(client_socket, &sessions).await;
    }

    #[tokio::test]
    async fn doesnt_do_anything_if_not_in_a_session_when_disconnect() {
        let mut client_socket = MockClientSocket::new();
        let mut sessions = MockSessionStore::new();

        client_socket
            .expect_get_stored_value()
            .times(1)
            .return_const(None);

        client_socket.expect_emit_to_room::<()>().never();
        client_socket.expect_remove_value().never();
        sessions.expect_delete_session().never();

        on_disconnect(client_socket, &sessions).await;
    }
}
