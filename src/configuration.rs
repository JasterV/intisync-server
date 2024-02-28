use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub port: u16,
    pub controller: ControllerConfig,
    pub redis: RedisConfig,
    pub session_ttl: Option<usize>,
}

#[derive(Clone, Copy, Deserialize)]
pub struct ControllerConfig {
    pub session_join_request_timeout: u64,
}

#[derive(Clone, Deserialize)]
pub struct RedisConfig {
    pub host: Arc<str>,
    pub port: u16,
    pub user: Arc<str>,
    pub password: Arc<str>,
    pub cache_pool_expire_seconds: u64,
    pub cache_pool_max_open: u64,
    pub cache_pool_max_idle: u64,
    pub cache_pool_timeout_seconds: u64,
}

impl RedisConfig {
    pub fn addr(&self) -> String {
        format!(
            "redis://{}:{}@{}:{}/",
            self.user.trim(),
            self.password.trim(),
            self.host.trim(),
            self.port
        )
    }
}

impl Config {
    pub fn load() -> Self {
        config::Config::builder()
            .add_source(
                config::Environment::default()
                    .try_parsing(true)
                    .separator("__"),
            )
            .build()
            .expect("Failed to load app configuration")
            .try_deserialize()
            .expect("Cannot deserialize configuration")
    }
}
