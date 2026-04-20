pub const CHUNK_SIZE: usize = 64;
const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

pub struct Chunk {
    pub blocks: Box<[u8; CHUNK_VOLUME]>,
}

impl Chunk {
    pub fn new() -> Self {
        let mut blocks = Box::new([0; CHUNK_VOLUME]);

        // Generate some varied terrain for testing materials
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    if y < 4 {
                        blocks[Self::index(x, y, z)] = 3; // Stone base
                    } else if y == 4 {
                        blocks[Self::index(x, y, z)] = 2; // Dirt layer
                    } else if y == 5 {
                        blocks[Self::index(x, y, z)] = 1; // grass
                    }
                }
            }
        }

        Self { blocks }
    }

    pub fn index(x: usize, y: usize, z: usize) -> usize {
        x + (y * CHUNK_SIZE) + (z * CHUNK_SIZE * CHUNK_SIZE)
    }

    pub fn get_block(&self, x: i32, y: i32, z: i32) -> u8 {
        if x < 0
            || x >= CHUNK_SIZE as i32
            || y < 0
            || y >= CHUNK_SIZE as i32
            || z < 0
            || z >= CHUNK_SIZE as i32
        {
            return 0;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)]
    }

    /// Safely sets a block's ID
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, id: u8) {
        if x < 0
            || x >= CHUNK_SIZE as i32
            || y < 0
            || y >= CHUNK_SIZE as i32
            || z < 0
            || z >= CHUNK_SIZE as i32
        {
            return; // Ignore clicks outside the chunk
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = id;
    }

    /// Fast Voxel Traversal Algorithm (DDA)
    /// Returns: Option<(Hit_Block_Position, Hit_Face_Normal)>
    pub fn raycast(
        &self,
        origin: glam::Vec3,
        dir: glam::Vec3,
        max_dist: f32,
    ) -> Option<(glam::IVec3, glam::IVec3)> {
        let mut t = 0.0;

        // The current voxel integer coordinate
        let mut current_pos = glam::IVec3::new(
            origin.x.floor() as i32,
            origin.y.floor() as i32,
            origin.z.floor() as i32,
        );

        // Which direction we step on each axis (+1 or -1)
        let step = glam::IVec3::new(
            if dir.x > 0.0 { 1 } else { -1 },
            if dir.y > 0.0 { 1 } else { -1 },
            if dir.z > 0.0 { 1 } else { -1 },
        );

        // How far we have to travel along the ray to move 1 unit on an axis
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

        // Distance along the ray to the next voxel boundary
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
            // Did we hit a solid block?
            if self.get_block(current_pos.x, current_pos.y, current_pos.z) != 0 {
                return Some((current_pos, hit_normal));
            }

            // Advance to the next voxel boundary along the shortest axis
            if t_max.x < t_max.y {
                if t_max.x < t_max.z {
                    current_pos.x += step.x;
                    t = t_max.x;
                    t_max.x += t_delta.x;
                    hit_normal = glam::IVec3::new(-step.x, 0, 0); // Normal is inverse of our step
                } else {
                    current_pos.z += step.z;
                    t = t_max.z;
                    t_max.z += t_delta.z;
                    hit_normal = glam::IVec3::new(0, 0, -step.z);
                }
            } else {
                if t_max.y < t_max.z {
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
        }
        None // Ray traveled max_dist without hitting anything
    }
}
