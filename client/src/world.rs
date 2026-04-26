use std::collections::HashMap;

use shared::{
    chunk::Chunk,
    protocol::{BlockUpdate, PlayerId, PlayerTransform, ServerMessage, WorldSnapshot},
};

use crate::player::{Player, RemotePlayer};

#[derive(Default, Debug)]
pub struct DirtyFlags {
    pub chunk: bool,
    pub remote_players: bool,
    pub highlight: bool,
}

impl DirtyFlags {
    pub fn any(&self) -> bool {
        self.chunk || self.remote_players || self.highlight
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

pub struct World {
    pub chunk: Chunk,
    pub player: Player,
    pub remote_players: HashMap<PlayerId, RemotePlayer>,
    pub target: Option<(glam::IVec3, glam::IVec3)>,
    pub dirty: DirtyFlags,
}

impl World {
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            player: Player::new(
                glam::Vec3::from_array(shared::constants::SPAWN_POSITION),
                glam::Quat::IDENTITY,
                1.0,
            ),
            remote_players: HashMap::new(),
            target: None,
            dirty: DirtyFlags {
                chunk: true,
                remote_players: true,
                highlight: true,
            },
        }
    }

    pub fn apply_server_message(&mut self, message: ServerMessage) -> Result<(), String> {
        match message {
            ServerMessage::Welcome {
                player_id,
                snapshot,
                ..
            } => {
                self.player.player_id = Some(player_id);
                self.apply_snapshot(player_id, snapshot)?;
            }
            ServerMessage::WorldUpdate {
                players,
                blocks,
                disconnected_players,
                ..
            } => {
                if self.apply_block_updates(&blocks) {
                    self.dirty.chunk = true;
                }
                if self.apply_player_updates(players, disconnected_players) {
                    self.dirty.remote_players = true;
                }
            }
            ServerMessage::Error { message } => return Err(message),
        }

        Ok(())
    }

    pub fn set_block_if_changed(&mut self, position: glam::IVec3, block_type: u8) -> bool {
        if self.chunk.get_block(position) == block_type {
            return false;
        }

        self.chunk.set_block(position, block_type);
        if self.chunk.get_block(position) != block_type {
            return false;
        }

        true
    }

    fn apply_snapshot(
        &mut self,
        player_id: PlayerId,
        snapshot: WorldSnapshot,
    ) -> Result<(), String> {
        if snapshot.chunk_blocks.len() != self.chunk.blocks.len() {
            return Err(format!(
                "server sent {} chunk blocks, expected {}",
                snapshot.chunk_blocks.len(),
                self.chunk.blocks.len()
            ));
        }

        let chunk_changed = self
            .chunk
            .blocks
            .iter()
            .zip(snapshot.chunk_blocks.iter())
            .any(|(current, next)| current != next);
        if chunk_changed {
            for (index, block) in snapshot.chunk_blocks.into_iter().enumerate() {
                self.chunk.blocks[index] = block;
            }
            self.dirty.chunk = true;
        }

        let mut next_remote_players = HashMap::new();
        for transform in snapshot.players {
            if transform.player_id == player_id {
                self.player.set_transform(
                    glam::Vec3::from_array(transform.position),
                    shared::math::quat_from_array(transform.rotation),
                );
            } else {
                next_remote_players.insert(
                    transform.player_id,
                    RemotePlayer {
                        position: glam::Vec3::from_array(transform.position),
                        rotation: shared::math::quat_from_array(transform.rotation),
                    },
                );
            }
        }

        if self.remote_players != next_remote_players {
            self.remote_players = next_remote_players;
            self.dirty.remote_players = true;
        }

        Ok(())
    }

    fn apply_block_updates(&mut self, blocks: &[BlockUpdate]) -> bool {
        let mut changed = false;

        for block in blocks {
            if self.set_block_if_changed(glam::IVec3::from_array(block.position), block.block_type)
            {
                changed = true;
            }
        }

        changed
    }

    fn apply_player_updates(
        &mut self,
        players: Vec<PlayerTransform>,
        disconnected_players: Vec<PlayerId>,
    ) -> bool {
        let mut changed = false;

        for player in players {
            if self.apply_player_transform(player) {
                changed = true;
            }
        }

        for player_id in disconnected_players {
            if self.remote_players.remove(&player_id).is_some() {
                changed = true;
            }
        }

        changed
    }

    fn apply_player_transform(&mut self, transform: PlayerTransform) -> bool {
        if Some(transform.player_id) == self.player.player_id {
            return false;
        }

        let remote = RemotePlayer {
            position: glam::Vec3::from_array(transform.position),
            rotation: shared::math::quat_from_array(transform.rotation),
        };

        if self.remote_players.get(&transform.player_id) == Some(&remote) {
            return false;
        }

        self.remote_players.insert(transform.player_id, remote);
        true
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::protocol::ServerMessage;

    #[test]
    fn same_value_block_update_does_not_dirty_chunk() {
        let mut world = World::new();
        world.dirty.clear();
        world.chunk.set_block(glam::IVec3::new(1, 2, 3), 4);

        world
            .apply_server_message(ServerMessage::WorldUpdate {
                tick: 1,
                players: Vec::new(),
                blocks: vec![BlockUpdate {
                    position: [1, 2, 3],
                    block_type: 4,
                }],
                disconnected_players: Vec::new(),
            })
            .unwrap();

        assert!(!world.dirty.chunk);
    }

    #[test]
    fn changed_block_update_dirties_chunk() {
        let mut world = World::new();
        world.dirty.clear();

        world
            .apply_server_message(ServerMessage::WorldUpdate {
                tick: 1,
                players: Vec::new(),
                blocks: vec![BlockUpdate {
                    position: [1, 2, 3],
                    block_type: 4,
                }],
                disconnected_players: Vec::new(),
            })
            .unwrap();

        assert!(world.dirty.chunk);
        assert_eq!(world.chunk.get_block(glam::IVec3::new(1, 2, 3)), 4);
    }

    #[test]
    fn snapshot_rejects_wrong_chunk_length() {
        let mut world = World::new();
        let result = world.apply_server_message(ServerMessage::Welcome {
            player_id: PlayerId(1),
            tick_hz: 30,
            snapshot: WorldSnapshot {
                players: Vec::new(),
                chunk_blocks: vec![0],
            },
        });

        assert!(result.is_err());
    }

    #[test]
    fn remote_player_disconnect_dirties_only_when_present() {
        let mut world = World::new();
        world.player.player_id = Some(PlayerId(1));
        world.remote_players.insert(
            PlayerId(2),
            RemotePlayer {
                position: glam::Vec3::ONE,
                rotation: glam::Quat::IDENTITY,
            },
        );
        world.dirty.clear();

        world
            .apply_server_message(ServerMessage::WorldUpdate {
                tick: 1,
                players: Vec::new(),
                blocks: Vec::new(),
                disconnected_players: vec![PlayerId(2)],
            })
            .unwrap();

        assert!(world.dirty.remote_players);

        world.dirty.clear();
        world
            .apply_server_message(ServerMessage::WorldUpdate {
                tick: 2,
                players: Vec::new(),
                blocks: Vec::new(),
                disconnected_players: vec![PlayerId(2)],
            })
            .unwrap();
        assert!(!world.dirty.remote_players);
    }

    #[test]
    fn own_player_transform_is_not_remote_dirty() {
        let mut world = World::new();
        world.player.player_id = Some(PlayerId(1));
        world.dirty.clear();

        world
            .apply_server_message(ServerMessage::WorldUpdate {
                tick: 1,
                players: vec![PlayerTransform {
                    player_id: PlayerId(1),
                    position: [3.0, 4.0, 5.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                }],
                blocks: Vec::new(),
                disconnected_players: Vec::new(),
            })
            .unwrap();

        assert!(!world.dirty.remote_players);
        assert!(world.remote_players.is_empty());
    }
}
