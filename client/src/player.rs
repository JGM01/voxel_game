use shared::protocol::PlayerId;
use web_time::{Duration, Instant};
use winit::{event::ElementState, keyboard::KeyCode};

use crate::camera::Camera;

const MOVE_SEND_INTERVAL: Duration = Duration::from_millis(shared::constants::MOVE_SEND_INTERVAL_MS);
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

#[derive(Debug)]
pub struct PlayerController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    speed: f32,
    sensitivity_x: f32,
    sensitivity_y: f32,
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
        self.mirror_camera();
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

    pub fn mirror_camera(&mut self) {
        self.camera.position = self.position;
        self.camera.rotation = self.rotation;
    }

    fn rotate(&mut self, yaw: f32, pitch: f32, roll: f32) {
        let mut delta = glam::Quat::IDENTITY;

        if yaw != 0.0 {
            let yaw = glam::Quat::from_axis_angle(glam::Vec3::Y, yaw);
            delta = yaw * delta;
        }
        if pitch != 0.0 {
            let pitch = glam::Quat::from_axis_angle(self.right(), pitch);
            delta *= pitch;
        }
        if roll != 0.0 {
            let roll = glam::Quat::from_axis_angle(self.forward(), roll);
            delta *= roll;
        }

        self.rotation = (delta * self.rotation).normalize();
    }
}

impl Default for PlayerController {
    fn default() -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            speed: 5.0,
            sensitivity_x: 0.2,
            sensitivity_y: 0.1,
        }
    }
}

impl PlayerController {
    pub fn new(speed: f32, sensitivity_x: f32, sensitivity_y: f32) -> Self {
        Self {
            speed,
            sensitivity_x,
            sensitivity_y,
            ..Default::default()
        }
    }

    pub fn process_keyboard(&mut self, key: KeyCode, state: ElementState) -> bool {
        let amount = if state == ElementState::Pressed {
            1.0
        } else {
            0.0
        };
        match key {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.amount_forward = amount;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.amount_backward = amount;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.amount_left = amount;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.amount_right = amount;
                true
            }
            KeyCode::Space => {
                self.amount_up = amount;
                true
            }
            KeyCode::ShiftLeft => {
                self.amount_down = amount;
                true
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    pub fn update_player(&mut self, player: &mut Player, dt: f32) {
        let mut forward_flat = player.forward();
        forward_flat.y = 0.0;
        forward_flat = forward_flat.normalize_or_zero();

        let mut right_flat = player.right();
        right_flat.y = 0.0;
        right_flat = right_flat.normalize_or_zero();

        player.position +=
            forward_flat * (self.amount_forward - self.amount_backward) * self.speed * dt;
        player.position += right_flat * (self.amount_right - self.amount_left) * self.speed * dt;
        player.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        player.rotate(
            self.rotate_horizontal * self.sensitivity_x * dt,
            self.rotate_vertical * self.sensitivity_y * dt,
            0.0,
        );

        player.mirror_camera();

        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;
    }
}
