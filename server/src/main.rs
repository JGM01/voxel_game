use std::net::SocketAddr;

use axum::{
    Router,
    extract::{ConnectInfo, State, WebSocketUpgrade, ws::WebSocket},
    response::IntoResponse,
    routing::any,
};
use axum_extra::{TypedHeader, headers};
use shared::chunk::Chunk;
use tokio::sync::{mpsc, oneshot};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug)]
struct World {
    /// POSITION IN WORLD COORDINATES
    pub player_positions: Vec<glam::Vec3>,

    /// ORIENTATION (UNIT QUATERNION)
    pub player_rotations: Vec<glam::Quat>,

    /// BLOCK DATA
    pub chunk: Chunk,
}

#[derive(Debug)]
struct PlayerId {
    index: usize,
}

#[derive(Debug)]
enum GameCommand {
    Connect {
        reply: oneshot::Sender<PlayerId>,
    },
    Disconnect {
        player_id: PlayerId,
    },
    MovePlayer {
        player_id: PlayerId,
        position: glam::Vec3,
        rotation: glam::Quat,
    },
    PlaceBlock {
        player_id: PlayerId,
        block_position: usize,
        block_type: u8,
    },
    BreakBlock {
        player_id: PlayerId,
        block_position: usize,
        block_type: u8,
    },
}

async fn game_task(mut rx: mpsc::Receiver<GameCommand>) {
    let mut world = World {
        player_positions: Vec::new(),
        player_rotations: Vec::new(),
        chunk: Chunk::new(),
    };

    while let Some(command) = rx.recv().await {
        match command {
            GameCommand::Connect { reply } => {
                let id = PlayerId {
                    index: world.player_positions.len(),
                };

                world
                    .player_positions
                    .push(glam::Vec3::new(8.0, 15.0, -15.0));
                world.player_rotations.push(glam::Quat::IDENTITY);

                let _ = reply.send(id);
            }
            GameCommand::Disconnect { player_id } => {
                println!("player disconnected: {:?}", player_id);
                // mark slot as free here
            }
            GameCommand::MovePlayer {
                player_id,
                position,
                rotation,
            } => {
                world.player_positions[player_id.index] = position;
                world.player_rotations[player_id.index] = rotation;
            }
            GameCommand::PlaceBlock {
                player_id,
                block_position,
                block_type,
            } => {
                println!(
                    "player {:?} placed block of type {:?}",
                    player_id, block_type
                );
                world.chunk.blocks[block_position] = block_type;
            }
            GameCommand::BreakBlock {
                player_id,
                block_position,
                block_type,
            } => {
                println!(
                    "player {:?} broke block of type {:?}",
                    player_id, block_type
                );
                world.chunk.blocks[block_position] = 0;
            }
        }
    }
}

#[derive(Clone)]
struct AppState {
    game_tx: mpsc::Sender<GameCommand>,
}

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

    let (game_tx, game_rx) = mpsc::channel::<GameCommand>(1024);

    tokio::spawn(game_task(game_rx));

    let state = AppState { game_tx };

    let app = Router::new()
        .route("/ws", any(ws_handler))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state.game_tx))
}

async fn handle_socket(mut socket: WebSocket, who: SocketAddr, game_tx: mpsc::Sender<GameCommand>) {
    let (reply_tx, reply_rx) = oneshot::channel();

    game_tx
        .send(GameCommand::Connect { reply: reply_tx })
        .await
        .unwrap();

    let player_id = reply_rx.await.unwrap();

    println!("{who} joined as {player_id:?}");
}
