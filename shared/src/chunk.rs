pub const CHUNK_SIZE: usize = 64;
const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

#[derive(Clone, Debug)]
pub struct Chunk {
    pub blocks: Box<[u8; CHUNK_VOLUME]>,
}

impl Chunk {
    pub fn new() -> Self {
        let mut blocks = Box::new([0; CHUNK_VOLUME]);

        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let pos = glam::UVec3::new(x as u32, y as u32, z as u32);

                    if y < 4 {
                        blocks[Self::index(pos)] = 3;
                    } else if y == 4 {
                        blocks[Self::index(pos)] = 2;
                    } else if y == 5 {
                        blocks[Self::index(pos)] = 1;
                    }
                }
            }
        }

        Self { blocks }
    }

    pub fn empty() -> Self {
        Self {
            blocks: Box::new([0; CHUNK_VOLUME]),
        }
    }

    pub fn index(pos: glam::UVec3) -> usize {
        pos.x as usize + (pos.y as usize * CHUNK_SIZE) + (pos.z as usize * CHUNK_SIZE * CHUNK_SIZE)
    }

    pub fn get_block(&self, pos: glam::IVec3) -> u8 {
        if pos.x < 0
            || pos.x >= CHUNK_SIZE as i32
            || pos.y < 0
            || pos.y >= CHUNK_SIZE as i32
            || pos.z < 0
            || pos.z >= CHUNK_SIZE as i32
        {
            return 0;
        }

        self.blocks[Self::index(pos.as_uvec3())]
    }

    pub fn set_block(&mut self, pos: glam::IVec3, id: u8) {
        if pos.x < 0
            || pos.x >= CHUNK_SIZE as i32
            || pos.y < 0
            || pos.y >= CHUNK_SIZE as i32
            || pos.z < 0
            || pos.z >= CHUNK_SIZE as i32
        {
            return;
        }

        self.blocks[Self::index(pos.as_uvec3())] = id;
    }

    pub fn raycast(
        &self,
        origin: glam::Vec3,
        dir: glam::Vec3,
        max_dist: f32,
    ) -> Option<(glam::IVec3, glam::IVec3)> {
        let mut t = 0.0;

        let mut current_pos = origin.floor().as_ivec3();

        let step = glam::IVec3::new(
            if dir.x > 0.0 { 1 } else { -1 },
            if dir.y > 0.0 { 1 } else { -1 },
            if dir.z > 0.0 { 1 } else { -1 },
        );

        let t_delta = glam::Vec3::new(
            if dir.x == 0.0 {
                f32::MAX
            } else {
                (1.0 / dir.x).abs()
            },
            if dir.y == 0.0 {
                f32::MAX
            } else {
                (1.0 / dir.y).abs()
            },
            if dir.z == 0.0 {
                f32::MAX
            } else {
                (1.0 / dir.z).abs()
            },
        );

        let mut t_max = glam::Vec3::new(
            if dir.x > 0.0 {
                (current_pos.x as f32 + 1.0 - origin.x) * t_delta.x
            } else {
                (origin.x - current_pos.x as f32) * t_delta.x
            },
            if dir.y > 0.0 {
                (current_pos.y as f32 + 1.0 - origin.y) * t_delta.y
            } else {
                (origin.y - current_pos.y as f32) * t_delta.y
            },
            if dir.z > 0.0 {
                (current_pos.z as f32 + 1.0 - origin.z) * t_delta.z
            } else {
                (origin.z - current_pos.z as f32) * t_delta.z
            },
        );

        let mut hit_normal = glam::IVec3::ZERO;

        while t < max_dist {
            if self.get_block(current_pos) != 0 {
                return Some((current_pos, hit_normal));
            }

            if t_max.x < t_max.y {
                if t_max.x < t_max.z {
                    current_pos.x += step.x;
                    t = t_max.x;
                    t_max.x += t_delta.x;
                    hit_normal = glam::IVec3::new(-step.x, 0, 0);
                } else {
                    current_pos.z += step.z;
                    t = t_max.z;
                    t_max.z += t_delta.z;
                    hit_normal = glam::IVec3::new(0, 0, -step.z);
                }
            } else if t_max.y < t_max.z {
                current_pos.y += step.y;
                t = t_max.y;
                t_max.y += t_delta.y;
                hit_normal = glam::IVec3::new(0, -step.y, 0);
            } else {
                current_pos.z += step.z;
                t = t_max.z;
                t_max.z += t_delta.z;
                hit_normal = glam::IVec3::new(0, 0, -step.z);
            }
        }

        None
    }
}
