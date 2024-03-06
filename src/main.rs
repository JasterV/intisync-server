mod actors;
mod configuration;
mod sessions;
mod socket;

use crate::{actors::Auth, configuration::Config};
use axum::routing::get;
use sessions::adapters::redis::{self, RedisSessionStore};
use socketioxide::{
    extract::{SocketRef, TryData},
    SocketIoBuilder,
};

#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    dotenvy::dotenv().ok();
    let config = Config::load();

    // WARN: Remember to use a subscriber if we ever stop relying on Shuttle
    // We don't use our own tracing subscriber now since Shuttle provides its own.
    // tracing::subscriber::set_global_default(FmtSubscriber::default())?;

    let pool = redis::pool::connect(&config.redis).expect("Couldn't connect to redis");

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

    let app = axum::Router::new()
        .route("/health-check", get(|| async { "ok" }))
        .layer(layer);

    Ok(app.into())
}
