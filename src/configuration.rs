use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub controller: ControllerConfig,
    #[serde(default)]
    pub redis: deadpool_redis::Config,
    pub session_ttl: Option<i64>,
}

#[derive(Clone, Copy, Deserialize)]
pub struct ControllerConfig {
    pub session_join_request_timeout: u64,
}

impl From<shuttle_secrets::SecretStore> for Config {
    fn from(value: shuttle_secrets::SecretStore) -> Self {
        let secrets = value.into_iter().collect::<HashMap<_, _>>();

        config::Config::builder()
            .add_source(
                config::Environment::default()
                    .source(Some(secrets))
                    .try_parsing(true)
                    .separator("__"),
            )
            .build()
            .expect("Failed to load app configuration")
            .try_deserialize()
            .expect("Cannot deserialize configuration")
    }
}

#[cfg(test)]
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
