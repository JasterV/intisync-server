use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub port: u16,
    pub controller: ControllerConfig,
    #[serde(default)]
    pub redis: deadpool_redis::Config,
    pub session_ttl: Option<i64>,
}

#[derive(Clone, Copy, Deserialize)]
pub struct ControllerConfig {
    pub session_join_request_timeout: u64,
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
