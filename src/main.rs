mod actors;
mod configuration;
mod sessions;
mod socket;

use crate::{actors::Auth, configuration::Config};
use sessions::adapters::redis::{self, RedisSessionStore};
use socketioxide::{
    extract::{SocketRef, TryData},
    SocketIoBuilder,
};
use tracing::info;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config = Config::load();

    tracing::subscriber::set_global_default(FmtSubscriber::default())?;

    let pool = redis::pool::connect(&config.redis)?;

    let sessions = RedisSessionStore::new(
        pool,
        redis::Config {
            session_ttl: config.session_ttl,
        },
    );

    let (layer, io) = SocketIoBuilder::new()
        .with_state(sessions)
        .with_state(config.clone())
        .build_layer();

    io.ns("/", {
        let io = io.clone();
        move |socket: SocketRef, auth: TryData<Auth>| {
            actors::on_connect::<RedisSessionStore>(socket, io.clone(), auth);
        }
    });

    let app = axum::Router::new().layer(layer);

    info!("Starting server");

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port))
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();

    Ok(())
}
