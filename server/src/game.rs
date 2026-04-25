use std::{collections::HashMap, time::Duration};

use shared::{
    chunk::Chunk,
    protocol::{BlockUpdate, PlayerId, PlayerTransform, ServerMessage, WorldSnapshot},
};
use tokio::{
    sync::{mpsc, oneshot},
    time,
};

pub(crate) const TICK_HZ: u64 = shared::constants::TICK_HZ;
const TICK_DURATION: Duration = Duration::from_millis(shared::constants::TICK_DURATION_MS);
const SPAWN_POSITION: glam::Vec3 = glam::Vec3::from_array(shared::constants::SPAWN_POSITION);

#[derive(Debug)]
struct World {
    players: HashMap<PlayerId, PlayerState>,
    next_player_id: u64,
    chunk: Chunk,
    tick: u64,
}

#[derive(Debug)]
struct PlayerState {
    position: glam::Vec3,
    rotation: glam::Quat,
    outbound: mpsc::Sender<ServerMessage>,
}

#[derive(Debug, Default)]
struct PendingChanges {
    players: Vec<DirtyPlayer>,
    blocks: Vec<DirtyBlock>,
    disconnected_players: Vec<PlayerId>,
}

#[derive(Debug)]
struct DirtyPlayer {
    player_id: PlayerId,
    source: PlayerId,
}

#[derive(Debug)]
struct DirtyBlock {
    update: BlockUpdate,
    source: PlayerId,
}

#[derive(Debug)]
pub(crate) enum GameCommand {
    Connect {
        outbound: mpsc::Sender<ServerMessage>,
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
        position: glam::IVec3,
        block_type: u8,
    },
    BreakBlock {
        player_id: PlayerId,
        position: glam::IVec3,
    },
}

pub(crate) async fn game_task(mut rx: mpsc::Receiver<GameCommand>) {
    let mut world = World {
        players: HashMap::new(),
        next_player_id: 1,
        chunk: Chunk::new(),
        tick: 0,
    };
    let mut pending = PendingChanges::default();
    let mut ticker = time::interval(TICK_DURATION);

    loop {
        tokio::select! {
            command = rx.recv() => {
                let Some(command) = command else {
                    break;
                };
                handle_game_command(command, &mut world, &mut pending).await;
            }
            _ = ticker.tick() => {
                broadcast_pending(&mut world, &mut pending).await;
            }
        }
    }
}

async fn handle_game_command(
    command: GameCommand,
    world: &mut World,
    pending: &mut PendingChanges,
) {
    match command {
        GameCommand::Connect { outbound, reply } => {
            let player_id = PlayerId(world.next_player_id);
            world.next_player_id += 1;

            let state = PlayerState {
                position: SPAWN_POSITION,
                rotation: glam::Quat::IDENTITY,
                outbound,
            };

            world.players.insert(player_id, state);

            let welcome = ServerMessage::Welcome {
                player_id,
                tick_hz: TICK_HZ,
                snapshot: world_snapshot(world),
            };
            if let Some(player) = world.players.get(&player_id) {
                let _ = player.outbound.send(welcome).await;
            }

            pending.players.push(DirtyPlayer {
                player_id,
                source: player_id,
            });
            let _ = reply.send(player_id);
        }
        GameCommand::Disconnect { player_id } => {
            if world.players.remove(&player_id).is_some() {
                pending.disconnected_players.push(player_id);
            }
        }
        GameCommand::MovePlayer {
            player_id,
            position,
            rotation,
        } => {
            if let Some(player) = world.players.get_mut(&player_id) {
                player.position = position;
                player.rotation = rotation.normalize();
                pending.players.push(DirtyPlayer {
                    player_id,
                    source: player_id,
                });
            }
        }
        GameCommand::PlaceBlock {
            player_id,
            position,
            block_type,
        } => {
            if world.players.contains_key(&player_id) && Chunk::contains(position) {
                world.chunk.set_block(position, block_type);
                pending.blocks.push(DirtyBlock {
                    update: BlockUpdate {
                        position: position.to_array(),
                        block_type,
                    },
                    source: player_id,
                });
            }
        }
        GameCommand::BreakBlock {
            player_id,
            position,
        } => {
            if world.players.contains_key(&player_id) && Chunk::contains(position) {
                world.chunk.set_block(position, shared::block::AIR);
                pending.blocks.push(DirtyBlock {
                    update: BlockUpdate {
                        position: position.to_array(),
                        block_type: shared::block::AIR,
                    },
                    source: player_id,
                });
            }
        }
    }
}

async fn broadcast_pending(world: &mut World, pending: &mut PendingChanges) {
    if pending.players.is_empty()
        && pending.blocks.is_empty()
        && pending.disconnected_players.is_empty()
    {
        return;
    }

    world.tick += 1;
    let tick = world.tick;
    let recipients: Vec<PlayerId> = world.players.keys().copied().collect();

    for recipient in recipients {
        let players = pending
            .players
            .iter()
            .filter(|dirty| dirty.source != recipient)
            .filter_map(|dirty| player_transform(dirty.player_id, world))
            .collect::<Vec<_>>();
        let blocks = pending
            .blocks
            .iter()
            .filter(|dirty| dirty.source != recipient)
            .map(|dirty| dirty.update.clone())
            .collect::<Vec<_>>();

        if players.is_empty() && blocks.is_empty() && pending.disconnected_players.is_empty() {
            continue;
        }

        let message = ServerMessage::WorldUpdate {
            tick,
            players,
            blocks,
            disconnected_players: pending.disconnected_players.clone(),
        };

        if let Some(player) = world.players.get(&recipient) {
            let _ = player.outbound.send(message).await;
        }
    }

    *pending = PendingChanges::default();
}

fn world_snapshot(world: &World) -> WorldSnapshot {
    WorldSnapshot {
        players: world
            .players
            .keys()
            .filter_map(|player_id| player_transform(*player_id, world))
            .collect(),
        chunk_blocks: world.chunk.blocks.iter().copied().collect(),
    }
}

fn player_transform(player_id: PlayerId, world: &World) -> Option<PlayerTransform> {
    let player = world.players.get(&player_id)?;
    Some(PlayerTransform {
        player_id,
        position: player.position.to_array(),
        rotation: shared::math::quat_to_array(player.rotation),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn connect_player(
        tx: &mpsc::Sender<GameCommand>,
    ) -> (PlayerId, mpsc::Receiver<ServerMessage>) {
        let (outbound_tx, mut outbound_rx) = mpsc::channel(16);
        let (reply_tx, reply_rx) = oneshot::channel();

        tx.send(GameCommand::Connect {
            outbound: outbound_tx,
            reply: reply_tx,
        })
        .await
        .unwrap();

        let player_id = reply_rx.await.unwrap();
        let welcome = outbound_rx.recv().await.unwrap();
        assert!(matches!(welcome, ServerMessage::Welcome { .. }));

        (player_id, outbound_rx)
    }

    async fn next_update_matching(
        rx: &mut mpsc::Receiver<ServerMessage>,
        matches: impl Fn(&ServerMessage) -> bool,
    ) -> ServerMessage {
        time::timeout(Duration::from_secs(1), async {
            loop {
                let message = rx.recv().await.unwrap();
                if matches(&message) {
                    return message;
                }
            }
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn connect_returns_unique_ids_and_welcome() {
        let (tx, rx) = mpsc::channel(16);
        let task = tokio::spawn(game_task(rx));

        let (first, _) = connect_player(&tx).await;
        let (second, _) = connect_player(&tx).await;

        assert_ne!(first, second);

        drop(tx);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn move_player_broadcasts_to_other_players() {
        let (tx, rx) = mpsc::channel(16);
        let task = tokio::spawn(game_task(rx));

        let (first, _) = connect_player(&tx).await;
        let (_, mut second_rx) = connect_player(&tx).await;

        tx.send(GameCommand::MovePlayer {
            player_id: first,
            position: glam::Vec3::new(1.0, 2.0, 3.0),
            rotation: glam::Quat::IDENTITY,
        })
        .await
        .unwrap();

        let update = next_update_matching(&mut second_rx, |message| {
            let ServerMessage::WorldUpdate { players, .. } = message else {
                return false;
            };
            players
                .iter()
                .any(|player| player.player_id == first && player.position == [1.0, 2.0, 3.0])
        })
        .await;
        let ServerMessage::WorldUpdate { players, .. } = update else {
            panic!("expected world update");
        };

        assert!(
            players
                .iter()
                .any(|player| { player.player_id == first && player.position == [1.0, 2.0, 3.0] })
        );

        drop(tx);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn block_changes_broadcast_to_other_players() {
        let (tx, rx) = mpsc::channel(16);
        let task = tokio::spawn(game_task(rx));

        let (first, _) = connect_player(&tx).await;
        let (_, mut second_rx) = connect_player(&tx).await;

        tx.send(GameCommand::PlaceBlock {
            player_id: first,
            position: glam::IVec3::new(1, 2, 3),
            block_type: 7,
        })
        .await
        .unwrap();

        let update = next_update_matching(&mut second_rx, |message| {
            let ServerMessage::WorldUpdate { blocks, .. } = message else {
                return false;
            };
            blocks
                == &vec![BlockUpdate {
                    position: [1, 2, 3],
                    block_type: 7,
                }]
        })
        .await;
        let ServerMessage::WorldUpdate { blocks, .. } = update else {
            panic!("expected world update");
        };

        assert_eq!(
            blocks,
            vec![BlockUpdate {
                position: [1, 2, 3],
                block_type: 7
            }]
        );

        tx.send(GameCommand::BreakBlock {
            player_id: first,
            position: glam::IVec3::new(1, 2, 3),
        })
        .await
        .unwrap();

        let update = next_update_matching(&mut second_rx, |message| {
            let ServerMessage::WorldUpdate { blocks, .. } = message else {
                return false;
            };
            blocks
                == &vec![BlockUpdate {
                    position: [1, 2, 3],
                    block_type: 0,
                }]
        })
        .await;
        let ServerMessage::WorldUpdate { blocks, .. } = update else {
            panic!("expected world update");
        };

        assert_eq!(
            blocks,
            vec![BlockUpdate {
                position: [1, 2, 3],
                block_type: 0
            }]
        );

        drop(tx);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn disconnect_broadcasts_to_remaining_players() {
        let (tx, rx) = mpsc::channel(16);
        let task = tokio::spawn(game_task(rx));

        let (first, _) = connect_player(&tx).await;
        let (_, mut second_rx) = connect_player(&tx).await;

        tx.send(GameCommand::Disconnect { player_id: first })
            .await
            .unwrap();

        let update = next_update_matching(&mut second_rx, |message| {
            let ServerMessage::WorldUpdate {
                disconnected_players,
                ..
            } = message
            else {
                return false;
            };
            disconnected_players == &vec![first]
        })
        .await;
        let ServerMessage::WorldUpdate {
            disconnected_players,
            ..
        } = update
        else {
            panic!("expected world update");
        };

        assert_eq!(disconnected_players, vec![first]);

        drop(tx);
        task.await.unwrap();
    }
}
