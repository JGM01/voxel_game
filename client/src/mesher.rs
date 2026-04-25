use shared::chunk::{CHUNK_SIZE, Chunk};

use crate::vertex::Vertex;

pub fn generate_mesh(chunk: &Chunk) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let pos = glam::UVec3::new(x as u32, y as u32, z as u32);
                let block_id = chunk.blocks[Chunk::index(pos)];

                if block_id == 0 {
                    continue;
                }

                let block_pos = pos.as_ivec3();

                let color = match block_id {
                    1 => [0.2, 0.8, 0.2],
                    2 => [0.4, 0.25, 0.1],
                    3 => [0.5, 0.5, 0.5],
                    _ => [1.0, 0.0, 1.0],
                };

                if chunk.get_block(block_pos + glam::IVec3::Y) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        pos,
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

                if chunk.get_block(block_pos - glam::IVec3::Y) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        pos,
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

                if chunk.get_block(block_pos + glam::IVec3::X) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        pos,
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

                if chunk.get_block(block_pos - glam::IVec3::X) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        pos,
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

                if chunk.get_block(block_pos + glam::IVec3::Z) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        pos,
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

                if chunk.get_block(block_pos - glam::IVec3::Z) == 0 {
                    add_face(
                        &mut vertices,
                        &mut indices,
                        pos,
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
    pos: glam::UVec3,
    color: [f32; 3],
    normal: [f32; 3],
    local_positions: [[f32; 3]; 4],
) {
    let base_index = vertices.len() as u32;
    let base_pos = pos.as_vec3();

    for local_pos in local_positions {
        vertices.push(Vertex::new(
            [
                local_pos[0] + base_pos.x,
                local_pos[1] + base_pos.y,
                local_pos[2] + base_pos.z,
            ],
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
