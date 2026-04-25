use std::net::SocketAddr;

use axum::{
    Router,
    extract::{ConnectInfo, State, WebSocketUpgrade, ws::Message, ws::WebSocket},
    response::IntoResponse,
    routing::any,
};
use axum_extra::{TypedHeader, headers};
use futures_util::{SinkExt, StreamExt};
use shared::protocol::{ClientMessage, PlayerId, ServerMessage};
use tokio::sync::{mpsc, oneshot};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use crate::game::GameCommand;

#[derive(Clone)]
struct AppState {
    game_tx: mpsc::Sender<GameCommand>,
}

// Defining my own router as to make main.rs feel cleaner lol
pub(crate) fn router(game_tx: mpsc::Sender<GameCommand>) -> Router {
    let state = AppState { game_tx };

    Router::new()
        .route("/ws", any(ws_handler))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
}

// This function is where HTTP becomes WebSockets. It has to be upgraded before the socket can be
// "handled" or so they say.
async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    // Grabbing some metadata about the type of browser, can be "Firefox", "Chrome", etc.
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    tracing::debug!("`{user_agent}` at {addr} connected.");

    // The actual upgrade, the socket is moved into the actual usage (along with game transmitter
    // stuff)
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state.game_tx))
}

// WebSocket state machine
async fn handle_socket(socket: WebSocket, who: SocketAddr, game_tx: mpsc::Sender<GameCommand>) {
    // multi-producer, single-consumer queue.
    // The multiple producers in this context are the clients, the server is the consumer
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<ServerMessage>(128);

    // sending single values across asynchronous tasks
    let (reply_tx, reply_rx) = oneshot::channel();

    // Try to register new client with game task by putting a GameCommand together and seeing if an
    // error is responded or not.
    if game_tx
        .send(GameCommand::Connect {
            outbound: outbound_tx.clone(),
            reply: reply_tx,
        })
        .await
        .is_err()
    {
        return;
    }

    // get the player id out of the reply
    let Ok(player_id) = reply_rx.await else {
        return;
    };

    tracing::debug!("{who} joined as {player_id:?}");

    // send/receive independently
    let (mut sender, mut receiver) = socket.split();

    // infinite loop
    loop {
        // cool tokio macro that lets multiple awaits wait at the same time, resolving to the first
        // hit. Because it's in an infinite loop, its just an infinite wait for whatever hit comes
        // around.
        tokio::select! {
            // has central game task dropped a message into outbound_rx?
            outbound = outbound_rx.recv() => {
                let Some(outbound) = outbound else {
                    break;
                };

                let Ok(text) = serde_json::to_string(&outbound) else {
                    continue;
                };

                if sender.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            // has client sent a WebSocket message?
            inbound = receiver.next() => {
                let Some(inbound) = inbound else {
                    break;
                };

                // figure out what the message is
                match inbound {
                    Ok(Message::Text(text)) => {
                        handle_client_text(&text, player_id, &game_tx, &outbound_tx).await;
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Binary(_)) => {
                        let _ = outbound_tx
                            .send(ServerMessage::Error {
                                message: "binary websocket messages are not supported".to_string(),
                            })
                            .await;
                    }
                    Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                    Err(error) => {
                        tracing::debug!("websocket error from {who}: {error}");
                        break;
                    }
                }
            }
        }
    }

    let _ = game_tx.send(GameCommand::Disconnect { player_id }).await;
    tracing::debug!("{who} disconnected from {player_id:?}");
}

async fn handle_client_text(
    text: &str,
    player_id: PlayerId,
    game_tx: &mpsc::Sender<GameCommand>,
    outbound_tx: &mpsc::Sender<ServerMessage>,
) {
    // get the JSON out of the string
    let message = match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => message,
        Err(error) => {
            let _ = outbound_tx
                .send(ServerMessage::Error {
                    message: format!("invalid client message: {error}"),
                })
                .await;
            return;
        }
    };

    let command = match message {
        ClientMessage::MovePlayer { position, rotation } => GameCommand::MovePlayer {
            player_id,
            position: glam::Vec3::from_array(position),
            rotation: shared::math::quat_from_array(rotation),
        },
        ClientMessage::PlaceBlock {
            position,
            block_type,
        } => GameCommand::PlaceBlock {
            player_id,
            position: glam::IVec3::from_array(position),
            block_type,
        },
        ClientMessage::BreakBlock { position } => GameCommand::BreakBlock {
            player_id,
            position: glam::IVec3::from_array(position),
        },
    };

    if game_tx.send(command).await.is_err() {
        let _ = outbound_tx
            .send(ServerMessage::Error {
                message: "game server is unavailable".to_string(),
            })
            .await;
    }
}
