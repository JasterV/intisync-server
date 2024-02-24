use mobc::Pool;
use mobc_redis::redis;
use mobc_redis::redis::AsyncCommands;
use mobc_redis::redis::FromRedisValue;
use mobc_redis::redis::RedisError;
use mobc_redis::RedisConnectionManager;
use std::time::Duration;

use crate::configuration::RedisConfig;

pub type MobcPool = Pool<RedisConnectionManager>;

#[derive(thiserror::Error, Debug)]
pub enum OperationError<T>
where
    T: std::error::Error + std::fmt::Debug,
{
    #[error("Error getting a connection from the pool: '{0}'")]
    GetConnection(mobc::Error<RedisError>),
    #[error(transparent)]
    ClientError(#[from] T),
}

#[derive(thiserror::Error, Debug)]
#[error("Failed to connect to redis: '{0}'")]
pub struct ConnectError(#[from] RedisError);

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

pub struct ConnectionConfig {
    cache_pool_expire_seconds: u64,
    cache_pool_max_open: u64,
    cache_pool_max_idle: u64,
    cache_pool_timeout_seconds: u64,
}

impl From<RedisConfig> for ConnectionConfig {
    fn from(value: RedisConfig) -> Self {
        Self {
            cache_pool_expire_seconds: value.cache_pool_expire_seconds,
            cache_pool_max_open: value.cache_pool_max_open,
            cache_pool_max_idle: value.cache_pool_max_idle,
            cache_pool_timeout_seconds: value.cache_pool_timeout_seconds,
        }
    }
}

pub fn connect(addr: &str, config: ConnectionConfig) -> Result<MobcPool, ConnectError> {
    let client = redis::Client::open(addr)?;
    // Make sure that we can connect to Redis
    let _connection = client.get_connection()?;
    let manager = RedisConnectionManager::new(client);
    let from_secs = Duration::from_secs(config.cache_pool_expire_seconds);
    Ok(Pool::builder()
        .get_timeout(Some(Duration::from_secs(config.cache_pool_timeout_seconds)))
        .max_open(config.cache_pool_max_open)
        .max_idle(config.cache_pool_max_idle)
        .max_lifetime(Some(from_secs))
        .build(manager))
}

pub async fn set_str(
    pool: &MobcPool,
    key: String,
    value: String,
    ttl_seconds: Option<usize>,
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
    pool: &MobcPool,
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

pub async fn exists(pool: &MobcPool, key: String) -> Result<bool, OperationError<ExistsError>> {
    let mut con = pool.get().await.map_err(OperationError::GetConnection)?;
    let value = con
        .exists(&key)
        .await
        .map_err(|err| ExistsError::Check(key.clone(), err))?;
    FromRedisValue::from_redis_value(&value).map_err(|e| ExistsError::ParseError(key, e).into())
}

pub async fn delete_key(pool: &MobcPool, key: String) -> Result<(), OperationError<DeleteError>> {
    let mut con = pool.get().await.map_err(OperationError::GetConnection)?;
    con.del(&key)
        .await
        .map_err(|err| DeleteError::Delete(key.clone(), err))?;
    Ok(())
}
