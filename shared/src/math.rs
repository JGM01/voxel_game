pub fn quat_to_array(rotation: glam::Quat) -> [f32; 4] {
    [rotation.x, rotation.y, rotation.z, rotation.w]
}

pub fn quat_from_array(rotation: [f32; 4]) -> glam::Quat {
    glam::Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]).normalize()
}
