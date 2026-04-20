use shared::chunk::{CHUNK_SIZE, Chunk};

use crate::vertex::Vertex;

pub fn generate_mesh(chunk: &Chunk) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let block_id = chunk.blocks[Chunk::index(x, y, z)];
                if block_id == 0 {
                    continue;
                }

                let color = match block_id {
                    1 => [0.2, 0.8, 0.2],  // Grass
                    2 => [0.4, 0.25, 0.1], // Dirt
                    3 => [0.5, 0.5, 0.5],  // Stone
                    _ => [1.0, 0.0, 1.0],
                };

                // TOP (+Y)
                if chunk.get_block(x as i32, y as i32 + 1, z as i32) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        z,
                        color,
                        [0.0, 1.0, 0.0],
                        [
                            [0.0, 1.0, 0.0],
                            [1.0, 1.0, 0.0],
                            [1.0, 1.0, 1.0],
                            [0.0, 1.0, 1.0],
                        ],
                    );
                }
                // BOTTOM (-Y)
                if chunk.get_block(x as i32, y as i32 - 1, z as i32) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        z,
                        color,
                        [0.0, -1.0, 0.0],
                        [
                            [0.0, 0.0, 1.0],
                            [1.0, 0.0, 1.0],
                            [1.0, 0.0, 0.0],
                            [0.0, 0.0, 0.0],
                        ],
                    );
                }
                // RIGHT (+X)
                if chunk.get_block(x as i32 + 1, y as i32, z as i32) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        z,
                        color,
                        [1.0, 0.0, 0.0],
                        [
                            [1.0, 0.0, 1.0],
                            [1.0, 1.0, 1.0],
                            [1.0, 1.0, 0.0],
                            [1.0, 0.0, 0.0],
                        ],
                    );
                }
                // LEFT (-X)
                if chunk.get_block(x as i32 - 1, y as i32, z as i32) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        z,
                        color,
                        [-1.0, 0.0, 0.0],
                        [
                            [0.0, 0.0, 0.0],
                            [0.0, 1.0, 0.0],
                            [0.0, 1.0, 1.0],
                            [0.0, 0.0, 1.0],
                        ],
                    );
                }
                // FRONT (+Z)
                if chunk.get_block(x as i32, y as i32, z as i32 + 1) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        z,
                        color,
                        [0.0, 0.0, 1.0],
                        [
                            [0.0, 0.0, 1.0],
                            [0.0, 1.0, 1.0],
                            [1.0, 1.0, 1.0],
                            [1.0, 0.0, 1.0],
                        ],
                    );
                }
                // BACK (-Z)
                if chunk.get_block(x as i32, y as i32, z as i32 - 1) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        z,
                        color,
                        [0.0, 0.0, -1.0],
                        [
                            [1.0, 0.0, 0.0],
                            [1.0, 1.0, 0.0],
                            [0.0, 1.0, 0.0],
                            [0.0, 0.0, 0.0],
                        ],
                    );
                }
            }
        }
    }
    (vertices, indices)
}

fn add_face(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    x: usize,
    y: usize,
    z: usize,
    color: [f32; 3],
    normal: [f32; 3],
    local_positions: [[f32; 3]; 4],
) {
    let base_index = vertices.len() as u32;
    for pos in local_positions {
        vertices.push(Vertex::new(
            [pos[0] + x as f32, pos[1] + y as f32, pos[2] + z as f32],
            normal,
            color,
        ));
    }
    indices.extend_from_slice(&[
        base_index,
        base_index + 1,
        base_index + 2,
        base_index,
        base_index + 2,
        base_index + 3,
    ]);
}
