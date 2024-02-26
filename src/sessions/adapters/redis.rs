pub mod pool;

use self::pool::MobcPool;
use crate::sessions::port::{
    CreateSessionError, DeleteSessionError, ExistsSessionError, GetSessionStateError, SessionState,
    SessionStore, UpdateSessionStateError,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct Config {
    pub session_ttl: Option<usize>,
}

#[derive(Clone)]
pub struct RedisSessionStore {
    pool: MobcPool,
    config: Config,
}

impl RedisSessionStore {
    pub fn new(pool: MobcPool, config: Config) -> Self {
        Self { pool, config }
    }
}

impl SessionStore for RedisSessionStore {
    async fn create_session(&self) -> Result<Uuid, CreateSessionError> {
        let id = Uuid::new_v4();
        if pool::exists(&self.pool, id.into())
            .await
            .map_err(Into::into)
            .map_err(CreateSessionError::IoError)?
        {
            return Err(CreateSessionError::UnexpectedSessionIdAlreadyInUse(id));
        }
        pool::set_str(
            &self.pool,
            id.to_string(),
            SessionState::WaitingForController.to_string(),
            self.config.session_ttl,
        )
        .await
        .map_err(Into::into)
        .map_err(CreateSessionError::IoError)?;
        Ok(id)
    }

    async fn delete_session(&self, id: Uuid) -> Result<(), DeleteSessionError> {
        pool::delete_key(&self.pool, id.into())
            .await
            .map_err(Into::into)
            .map_err(DeleteSessionError::IoError)?;
        Ok(())
    }

    async fn session_state(&self, id: Uuid) -> Result<Option<SessionState>, GetSessionStateError> {
        let value = pool::get_str(&self.pool, id.into())
            .await
            .map_err(Into::into)
            .map_err(GetSessionStateError::IoError)?
            .map(|value| SessionState::try_from(value).unwrap());
        Ok(value)
    }

    async fn exists_session(&self, id: Uuid) -> Result<bool, ExistsSessionError> {
        let value = pool::exists(&self.pool, id.into())
            .await
            .map_err(Into::into)
            .map_err(ExistsSessionError::IoError)?;
        Ok(value)
    }

    async fn update_session_state(
        &self,
        id: Uuid,
        state: SessionState,
    ) -> Result<(), UpdateSessionStateError> {
        if !pool::exists(&self.pool, id.into())
            .await
            .map_err(Into::into)
            .map_err(UpdateSessionStateError::IoError)?
        {
            return Err(UpdateSessionStateError::UnknownSession(id));
        };
        pool::set_str(
            &self.pool,
            id.to_string(),
            state.to_string(),
            self.config.session_ttl,
        )
        .await
        .map_err(Into::into)
        .map_err(UpdateSessionStateError::IoError)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use test_context::{test_context, AsyncTestContext};

    use super::RedisSessionStore;
    use crate::{
        configuration::Config,
        sessions::{
            adapters::redis::pool,
            port::{SessionState, SessionStore},
        },
    };

    impl AsyncTestContext for RedisSessionStore {
        async fn setup() -> Self {
            let config = Config::load();
            let pool = pool::connect(&config.redis.addr(), config.redis.into())
                .expect("Can't connect to redis");
            RedisSessionStore::new(
                pool,
                super::Config {
                    session_ttl: config.session_ttl,
                },
            )
        }

        async fn teardown(self) {}
    }

    #[test_context(RedisSessionStore)]
    #[tokio::test]
    async fn can_create_session(store: &mut RedisSessionStore) {
        let uuid = store.create_session().await.unwrap();
        assert!(store.exists_session(uuid).await.unwrap());
    }

    #[test_context(RedisSessionStore)]
    #[tokio::test]
    async fn can_delete_session(store: &mut RedisSessionStore) {
        let uuid = store.create_session().await.unwrap();
        assert!(store.exists_session(uuid).await.unwrap());
        assert!(store.delete_session(uuid).await.is_ok());
        assert!(!store.exists_session(uuid).await.unwrap());
    }

    #[test_context(RedisSessionStore)]
    #[tokio::test]
    async fn create_session_with_correct_initial_state(store: &mut RedisSessionStore) {
        let uuid = store.create_session().await.unwrap();
        assert_eq!(
            store.session_state(uuid).await.unwrap().unwrap(),
            SessionState::WaitingForController
        );
    }

    #[test_context(RedisSessionStore)]
    #[tokio::test]
    async fn can_update_session_state(store: &mut RedisSessionStore) {
        let uuid = store.create_session().await.unwrap();
        assert!(store
            .update_session_state(uuid, SessionState::InProgress)
            .await
            .is_ok());
        assert_eq!(
            store.session_state(uuid).await.unwrap().unwrap(),
            SessionState::InProgress
        );
    }
}
