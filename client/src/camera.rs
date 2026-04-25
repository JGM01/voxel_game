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
