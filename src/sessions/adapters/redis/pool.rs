use deadpool::managed::PoolError;
use deadpool::Runtime;
use deadpool_redis::redis::{self, AsyncCommands, FromRedisValue, RedisError};

pub type RedisPool = deadpool_redis::Pool;

#[derive(thiserror::Error, Debug)]
pub enum OperationError<T>
where
    T: std::error::Error + std::fmt::Debug,
{
    #[error("Error getting a connection from the pool: '{0}'")]
    GetConnection(PoolError<RedisError>),
    #[error(transparent)]
    ClientError(#[from] T),
}

#[derive(thiserror::Error, Debug)]
#[error("Failed to create Redis pool: '{0}'")]
pub struct ConnectError(#[from] deadpool_redis::CreatePoolError);

#[derive(thiserror::Error, Debug)]
#[error("Failed to set value '{1}' to key '{0}': '{2}'")]
pub struct SetError(String, String, RedisError);

#[derive(thiserror::Error, Debug)]
pub enum GetError {
    #[error("Failed to get value from key '{0}': '{1}'")]
    GetValue(String, RedisError),
    #[error("Error parsing redis value to a string from key '{0}': '{1}'")]
    ParseError(String, RedisError),
}

#[derive(thiserror::Error, Debug)]
pub enum ExistsError {
    #[error("Failed to check if a value exists with key '{0}': '{1}'")]
    Check(String, RedisError),
    #[error("Error parsing redis value to a bool from key '{0}': '{1}'")]
    ParseError(String, RedisError),
}

#[derive(thiserror::Error, Debug)]
pub enum DeleteError {
    #[error("Failed to delete key '{0}': '{1}'")]
    Delete(String, RedisError),
}

pub fn connect(config: &deadpool_redis::Config) -> Result<RedisPool, ConnectError> {
    let pool = config.create_pool(Some(Runtime::Tokio1))?;
    Ok(pool)
}

pub async fn set_str(
    pool: &RedisPool,
    key: String,
    value: String,
    ttl_seconds: Option<i64>,
) -> Result<(), OperationError<SetError>> {
    let mut con = pool.get().await.map_err(OperationError::GetConnection)?;
    con.set(&key, &value)
        .await
        .map_err(|err| SetError(key.clone(), value.clone(), err))?;
    if ttl_seconds.is_some_and(|x| x > 0) {
        con.expire(&key, ttl_seconds.unwrap())
            .await
            .map_err(|err| SetError(key, value, err))?;
    }
    Ok(())
}

pub async fn get_str(
    pool: &RedisPool,
    key: String,
) -> Result<Option<String>, OperationError<GetError>> {
    let mut con = pool.get().await.map_err(OperationError::GetConnection)?;
    let value: redis::Value = con
        .get(&key)
        .await
        .map_err(|err| GetError::GetValue(key.clone(), err))?;
    match value {
        redis::Value::Nil => Ok(None),
        _ => FromRedisValue::from_redis_value(&value)
            .map(Some)
            .map_err(|e| GetError::ParseError(key, e).into()),
    }
}

pub async fn exists(pool: &RedisPool, key: String) -> Result<bool, OperationError<ExistsError>> {
    let mut con = pool.get().await.map_err(OperationError::GetConnection)?;
    let value = con
        .exists(&key)
        .await
        .map_err(|err| ExistsError::Check(key.clone(), err))?;
    FromRedisValue::from_redis_value(&value).map_err(|e| ExistsError::ParseError(key, e).into())
}

pub async fn delete_key(pool: &RedisPool, key: String) -> Result<(), OperationError<DeleteError>> {
    let mut con = pool.get().await.map_err(OperationError::GetConnection)?;
    con.del(&key)
        .await
        .map_err(|err| DeleteError::Delete(key.clone(), err))?;
    Ok(())
}
