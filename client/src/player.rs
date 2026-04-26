use shared::protocol::PlayerId;
use web_time::{Duration, Instant};

use crate::camera::Camera;

const MOVE_SEND_INTERVAL: Duration =
    Duration::from_millis(shared::constants::MOVE_SEND_INTERVAL_MS);
const POSITION_EPSILON_SQUARED: f32 = 0.0001;
const ROTATION_EPSILON: f32 = 0.0001;

pub struct Player {
    pub player_id: Option<PlayerId>,
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub camera: Camera,
    last_sent_position: glam::Vec3,
    last_sent_rotation: glam::Quat,
    last_move_sent: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemotePlayer {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
}

impl Player {
    pub fn new(position: glam::Vec3, rotation: glam::Quat, aspect: f32) -> Self {
        let camera = Camera {
            position,
            rotation,
            aspect,
            fov: 80.0,
            near: 0.1,
            far: 1000.0,
        };

        Self {
            player_id: None,
            position,
            rotation,
            camera,
            last_sent_position: position,
            last_sent_rotation: rotation,
            last_move_sent: None,
        }
    }

    pub fn set_transform(&mut self, position: glam::Vec3, rotation: glam::Quat) {
        self.position = position;
        self.rotation = rotation.normalize();
        self.last_sent_position = self.position;
        self.last_sent_rotation = self.rotation;
        self.sync_camera();
    }

    pub fn forward(&self) -> glam::Vec3 {
        self.rotation * glam::Vec3::Z
    }

    pub fn right(&self) -> glam::Vec3 {
        self.rotation * glam::Vec3::X
    }

    pub fn should_send_move(&mut self) -> bool {
        let now = Instant::now();
        if self
            .last_move_sent
            .is_some_and(|last| now.duration_since(last) < MOVE_SEND_INTERVAL)
        {
            return false;
        }

        let position_changed =
            self.position.distance_squared(self.last_sent_position) > POSITION_EPSILON_SQUARED;
        let rotation_changed =
            self.rotation.dot(self.last_sent_rotation).abs() < 1.0 - ROTATION_EPSILON;

        if !position_changed && !rotation_changed {
            return false;
        }

        self.last_move_sent = Some(now);
        self.last_sent_position = self.position;
        self.last_sent_rotation = self.rotation;
        true
    }

    pub fn sync_camera(&mut self) {
        self.camera.position = self.position;
        self.camera.rotation = self.rotation;
    }
}
