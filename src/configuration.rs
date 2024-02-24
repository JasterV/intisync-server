use std::net::Ipv4Addr;

use serde::Deserialize;

#[derive(Clone, Copy, Deserialize)]
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

#[derive(Clone, Copy, Deserialize)]
pub struct RedisConfig {
    pub host: Ipv4Addr,
    pub port: u16,
    pub cache_pool_expire_seconds: u64,
    pub cache_pool_max_open: u64,
    pub cache_pool_max_idle: u64,
    pub cache_pool_timeout_seconds: u64,
}

impl RedisConfig {
    pub fn addr(&self) -> String {
        format!("redis://localhost:{}/", self.port)
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
