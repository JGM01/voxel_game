use shared::protocol::ClientMessage;

use crate::{
    input::{FrameInput, Interaction},
    world::World,
};

const SPEED: f32 = 15.0;
const SENSITIVITY_X: f32 = 0.2;
const SENSITIVITY_Y: f32 = 0.1;
const INTERACTION_DISTANCE: f32 = 10.0;

pub fn tick(world: &mut World, input: &FrameInput, dt: f32) -> Vec<ClientMessage> {
    let mut outbound = Vec::new();

    integrate_player(world, input, dt);
    update_target(world);

    if let Some(interaction) = input.interact {
        if let Some(message) = apply_interaction(world, interaction) {
            outbound.push(message);
        }
    }

    if world.player.player_id.is_some() && world.player.should_send_move() {
        outbound.push(ClientMessage::MovePlayer {
            position: world.player.position.to_array(),
            rotation: shared::math::quat_to_array(world.player.rotation),
        });
    }

    outbound
}

fn integrate_player(world: &mut World, input: &FrameInput, dt: f32) {
    let player = &mut world.player;

    let flat_forward = {
        let forward = player.forward();
        glam::Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero()
    };
    let flat_right = {
        let right = player.right();
        glam::Vec3::new(right.x, 0.0, right.z).normalize_or_zero()
    };

    player.position += flat_forward * input.move_dir.z * SPEED * dt;
    player.position += flat_right * input.move_dir.x * SPEED * dt;
    player.position.y += input.move_dir.y * SPEED * dt;

    let yaw = glam::Quat::from_axis_angle(glam::Vec3::Y, input.look_delta.x * SENSITIVITY_X * dt);
    let pitch =
        glam::Quat::from_axis_angle(player.right(), input.look_delta.y * SENSITIVITY_Y * dt);
    player.rotation = (yaw * pitch * player.rotation).normalize();
    player.sync_camera();
}

fn update_target(world: &mut World) {
    let new_target = world.chunk.raycast(
        world.player.position,
        world.player.forward(),
        INTERACTION_DISTANCE,
    );

    if new_target != world.target {
        world.target = new_target;
        world.dirty.highlight = true;
    }
}

fn apply_interaction(world: &mut World, interaction: Interaction) -> Option<ClientMessage> {
    let (hit_pos, hit_normal) = world.chunk.raycast(
        world.player.position,
        world.player.forward(),
        INTERACTION_DISTANCE,
    )?;

    match interaction {
        Interaction::Break => {
            if !world.set_block_if_changed(hit_pos, shared::block::AIR) {
                return None;
            }

            world.dirty.chunk = true;
            Some(ClientMessage::BreakBlock {
                position: hit_pos.to_array(),
            })
        }
        Interaction::Place => {
            let place_pos = hit_pos + hit_normal;
            if !world.set_block_if_changed(place_pos, shared::block::STONE) {
                return None;
            }

            world.dirty.chunk = true;
            Some(ClientMessage::PlaceBlock {
                position: place_pos.to_array(),
                block_type: shared::block::STONE,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::DirtyFlags;

    #[test]
    fn movement_updates_player_and_camera() {
        let mut world = World::new();
        let input = FrameInput {
            move_dir: glam::Vec3::Z,
            look_delta: glam::Vec2::ZERO,
            interact: None,
        };

        tick(&mut world, &input, 1.0);

        assert!(world.player.position.z > shared::constants::SPAWN_POSITION[2]);
        assert_eq!(world.player.camera.position, world.player.position);
    }

    #[test]
    fn pitch_only_look_does_not_change_horizontal_forward_direction() {
        let mut world = World::new();
        world.player.rotation = glam::Quat::IDENTITY;
        world.player.sync_camera();

        tick(
            &mut world,
            &FrameInput {
                move_dir: glam::Vec3::ZERO,
                look_delta: glam::Vec2::Y,
                interact: None,
            },
            1.0,
        );

        let forward = world.player.forward();
        let horizontal = glam::Vec2::new(forward.x, forward.z).normalize_or_zero();
        assert!(horizontal.abs_diff_eq(glam::Vec2::Y, 0.0001));
    }

    #[test]
    fn highlight_dirty_only_when_target_changes() {
        let mut world = World::new();
        world.chunk = shared::chunk::Chunk::empty();
        world.player.position = glam::Vec3::new(1.5, 1.5, 0.5);
        world.player.rotation = glam::Quat::IDENTITY;
        world.player.sync_camera();
        world.chunk.set_block(glam::IVec3::new(1, 1, 2), 4);
        world.dirty.clear();
        let input = FrameInput {
            move_dir: glam::Vec3::ZERO,
            look_delta: glam::Vec2::ZERO,
            interact: None,
        };

        tick(&mut world, &input, 0.0);
        let first_dirty = world.dirty.highlight;
        world.dirty.clear();
        tick(&mut world, &input, 0.0);

        assert!(first_dirty);
        assert!(!world.dirty.highlight);
    }

    #[test]
    fn break_interaction_sets_chunk_dirty_and_emits_message() {
        let mut world = World::new();
        world.chunk = shared::chunk::Chunk::empty();
        world.player.position = glam::Vec3::new(1.5, 1.5, 0.5);
        world.player.rotation = glam::Quat::IDENTITY;
        world.player.sync_camera();
        world.chunk.set_block(glam::IVec3::new(1, 1, 2), 4);
        world.dirty = DirtyFlags::default();

        let messages = tick(
            &mut world,
            &FrameInput {
                move_dir: glam::Vec3::ZERO,
                look_delta: glam::Vec2::ZERO,
                interact: Some(Interaction::Break),
            },
            0.0,
        );

        assert!(world.dirty.chunk);
        assert_eq!(
            messages,
            vec![ClientMessage::BreakBlock {
                position: [1, 1, 2]
            }]
        );
    }

    #[test]
    fn place_interaction_noops_when_block_is_unchanged() {
        let mut world = World::new();
        world.chunk = shared::chunk::Chunk::empty();
        world.player.position = glam::Vec3::new(1.5, 1.5, 0.5);
        world.player.rotation = glam::Quat::IDENTITY;
        world.player.sync_camera();
        world.chunk.set_block(glam::IVec3::new(1, 1, 2), 4);
        world
            .chunk
            .set_block(glam::IVec3::new(1, 1, 0), shared::block::STONE);
        world.dirty = DirtyFlags::default();

        let messages = tick(
            &mut world,
            &FrameInput {
                move_dir: glam::Vec3::ZERO,
                look_delta: glam::Vec2::ZERO,
                interact: Some(Interaction::Place),
            },
            0.0,
        );

        assert!(!world.dirty.chunk);
        assert!(messages.is_empty());
    }
}
