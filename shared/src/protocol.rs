use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    MovePlayer {
        position: [f32; 3],
        rotation: [f32; 4],
    },
    PlaceBlock {
        position: [i32; 3],
        block_type: u8,
    },
    BreakBlock {
        position: [i32; 3],
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        player_id: PlayerId,
        tick_hz: u64,
        snapshot: WorldSnapshot,
    },
    WorldUpdate {
        tick: u64,
        players: Vec<PlayerTransform>,
        blocks: Vec<BlockUpdate>,
        disconnected_players: Vec<PlayerId>,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub players: Vec<PlayerTransform>,
    pub chunk_blocks: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerTransform {
    pub player_id: PlayerId,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockUpdate {
    pub position: [i32; 3],
    pub block_type: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_message_round_trips_as_json() {
        let message = ClientMessage::MovePlayer {
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };

        let json = serde_json::to_string(&message).unwrap();
        let decoded: ClientMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn server_message_round_trips_as_json() {
        let message = ServerMessage::WorldUpdate {
            tick: 7,
            players: vec![PlayerTransform {
                player_id: PlayerId(3),
                position: [8.0, 15.0, -15.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
            }],
            blocks: vec![BlockUpdate {
                position: [1, 2, 3],
                block_type: 4,
            }],
            disconnected_players: vec![PlayerId(2)],
        };

        let json = serde_json::to_string(&message).unwrap();
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, message);
    }
}
