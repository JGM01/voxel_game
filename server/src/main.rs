use std::net::SocketAddr;

use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod game;
mod net;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let (game_tx, game_rx) = mpsc::channel::<game::GameCommand>(1024);
    tokio::spawn(game::game_task(game_rx));

    let app = net::router(game_tx);
    let bind_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:3000".to_string())
        .parse::<SocketAddr>()
        .unwrap_or_else(|error| {
            eprintln!("invalid bind address; use an address like 127.0.0.1:3000 or 0.0.0.0:3000");
            eprintln!("{error}");
            std::process::exit(2);
        });

    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
