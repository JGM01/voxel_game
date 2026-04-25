pub fn server_url_from_arg(arg: Option<String>) -> Result<String, String> {
    let Some(arg) = arg else {
        return Ok("ws://127.0.0.1:3000/ws".to_string());
    };

    let arg = arg.trim();
    if arg.is_empty() {
        return Err("server address cannot be empty".to_string());
    }

    if arg.starts_with("ws://") || arg.starts_with("wss://") {
        return Ok(arg.to_string());
    }

    if let Ok(port) = arg.parse::<u16>() {
        return Ok(format!("ws://127.0.0.1:{port}/ws"));
    }

    if arg.contains(':') {
        return Ok(format!("ws://{arg}/ws"));
    }

    Err(format!(
        "invalid server address `{arg}`; use a port, host:port, or ws://host:port/ws"
    ))
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{sync::mpsc, thread};

    use futures::{SinkExt, StreamExt};
    use shared::protocol::{ClientMessage, ServerMessage};
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    #[derive(Debug)]
    pub enum NetworkEvent {
        Message(ServerMessage),
        Fatal(String),
    }

    pub struct NetworkClient {
        incoming: mpsc::Receiver<NetworkEvent>,
        outgoing: tokio::sync::mpsc::UnboundedSender<ClientMessage>,
    }

    impl NetworkClient {
        pub fn connect(url: String) -> Self {
            let (incoming_tx, incoming_rx) = mpsc::channel();
            let (outgoing_tx, outgoing_rx) = tokio::sync::mpsc::unbounded_channel();

            thread::spawn(move || {
                let Ok(runtime) = tokio::runtime::Runtime::new() else {
                    let _ = incoming_tx.send(NetworkEvent::Fatal(
                        "failed to start network runtime".to_string(),
                    ));
                    return;
                };

                runtime.block_on(run_socket(url, incoming_tx, outgoing_rx));
            });

            Self {
                incoming: incoming_rx,
                outgoing: outgoing_tx,
            }
        }

        pub fn try_recv(&self) -> Result<NetworkEvent, mpsc::TryRecvError> {
            self.incoming.try_recv()
        }

        pub fn send(&self, message: ClientMessage) {
            let _ = self.outgoing.send(message);
        }
    }

    async fn run_socket(
        url: String,
        incoming_tx: mpsc::Sender<NetworkEvent>,
        mut outgoing_rx: tokio::sync::mpsc::UnboundedReceiver<ClientMessage>,
    ) {
        let socket = match connect_async(&url).await {
            Ok((socket, _)) => socket,
            Err(error) => {
                let _ = incoming_tx.send(NetworkEvent::Fatal(format!(
                    "failed to connect to {url}: {error}"
                )));
                return;
            }
        };

        let (mut writer, mut reader) = socket.split();

        loop {
            tokio::select! {
                outbound = outgoing_rx.recv() => {
                    let Some(outbound) = outbound else {
                        return;
                    };

                    let Ok(text) = serde_json::to_string(&outbound) else {
                        continue;
                    };

                    if let Err(error) = writer.send(Message::Text(text.into())).await {
                        let _ = incoming_tx.send(NetworkEvent::Fatal(format!(
                            "failed to send websocket message: {error}"
                        )));
                        return;
                    }
                }
                inbound = reader.next() => {
                    let Some(inbound) = inbound else {
                        let _ = incoming_tx.send(NetworkEvent::Fatal(
                            "websocket closed by server".to_string(),
                        ));
                        return;
                    };

                    match inbound {
                        Ok(Message::Text(text)) => match serde_json::from_str::<ServerMessage>(&text) {
                            Ok(message) => {
                                let _ = incoming_tx.send(NetworkEvent::Message(message));
                            }
                            Err(error) => {
                                let _ = incoming_tx.send(NetworkEvent::Fatal(format!(
                                    "invalid server message: {error}"
                                )));
                                return;
                            }
                        },
                        Ok(Message::Close(_)) => {
                            let _ = incoming_tx.send(NetworkEvent::Fatal(
                                "websocket closed by server".to_string(),
                            ));
                            return;
                        }
                        Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                        Ok(Message::Binary(_)) | Ok(Message::Frame(_)) => {}
                        Err(error) => {
                            let _ = incoming_tx.send(NetworkEvent::Fatal(format!(
                                "websocket receive error: {error}"
                            )));
                            return;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{NetworkClient, NetworkEvent};

#[cfg(test)]
mod tests {
    use super::server_url_from_arg;

    #[test]
    fn defaults_to_localhost_server() {
        assert_eq!(server_url_from_arg(None).unwrap(), "ws://127.0.0.1:3000/ws");
    }

    #[test]
    fn accepts_port_only() {
        assert_eq!(
            server_url_from_arg(Some("4000".to_string())).unwrap(),
            "ws://127.0.0.1:4000/ws"
        );
    }

    #[test]
    fn accepts_host_port() {
        assert_eq!(
            server_url_from_arg(Some("100.64.1.2:3000".to_string())).unwrap(),
            "ws://100.64.1.2:3000/ws"
        );
    }

    #[test]
    fn accepts_full_url() {
        assert_eq!(
            server_url_from_arg(Some("wss://game.example.com/ws".to_string())).unwrap(),
            "wss://game.example.com/ws"
        );
    }
}
