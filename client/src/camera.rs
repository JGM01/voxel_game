use winit::event::ElementState;
use winit::keyboard::KeyCode;

pub struct Camera {
    /// POSITION IN WORLD COORDINATES
    pub position: glam::Vec3,

    /// ORIENTATION (UNIT QUATERNION)
    pub rotation: glam::Quat,

    /// WIDTH/HEIGHT
    pub aspect: f32,

    /// FOR PERSPECTIVE PROJECTION (RADIANS)
    pub fov: f32,

    /// DISTANCE TO NEAR PLANE (POSITIVE!)
    pub near: f32,

    /// DISTANCE TO FAR PLANE (POSITIVE, >NEAR!)
    pub far: f32,
}

#[derive(Debug)]
pub struct CameraController {
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

impl Default for CameraController {
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

impl CameraController {
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

    pub fn update_camera(&mut self, camera: &mut Camera, dt: f32) {
        let mut forward_flat = camera.forward();
        forward_flat.y = 0.0;
        forward_flat = forward_flat.normalize_or_zero();

        let mut right_flat = camera.right();
        right_flat.y = 0.0;
        right_flat = right_flat.normalize_or_zero();

        camera.position +=
            forward_flat * (self.amount_forward - self.amount_backward) * self.speed * dt;
        camera.position += right_flat * (self.amount_right - self.amount_left) * self.speed * dt;
        camera.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        camera.rotate(
            self.rotate_horizontal * self.sensitivity_x * dt,
            self.rotate_vertical * self.sensitivity_y * dt,
            0.0,
        );

        // Reset mouse deltas after applying
        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;
    }
}

impl Camera {
    pub fn build_view_projection_matrix(&self) -> glam::Mat4 {
        let view = self.view_matrix();
        let proj = self.projection_matrix();
        proj * view
    }

    pub fn view_matrix(&self) -> glam::Mat4 {
        let rotation_matrix = glam::Mat4::from_quat(self.rotation);
        let translation_matrix = glam::Mat4::from_translation(-self.position);

        // No more weird conversions!
        rotation_matrix.inverse() * translation_matrix
    }

    pub fn projection_matrix(&self) -> glam::Mat4 {
        // glam's perspective_lh matches wgpu's expected coordinate system
        // (Left-handed, with Z going from 0.0 to 1.0)
        glam::Mat4::perspective_lh(self.fov.to_radians(), self.aspect, self.near, self.far)
    }

    /// Computes forward direction vector (derived from self.rotation quaternion)
    /// - Works by rotating the "World Forward" (Z) by the camera's rotation.
    pub fn forward(&self) -> glam::Vec3 {
        self.rotation * glam::Vec3::Z // World Forward
    }

    /// Computes right direction vector
    /// - Rotate "World Right" (+X) by camera's rotation.
    pub fn right(&self) -> glam::Vec3 {
        self.rotation * glam::Vec3::X // World Right
    }

    /// Computes up direction vector
    /// - Rotate "World Up" (+Y) by camera's rotation.
    pub fn up(&self) -> glam::Vec3 {
        self.rotation * glam::Vec3::Y // World Up
    }

    /// Yaw around world Y, pitch around local right, roll around local forward.
    pub fn rotate(&mut self, yaw: f32, pitch: f32, roll: f32) {
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
