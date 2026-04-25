use shared::{
    chunk::Chunk,
    protocol::{
        BlockUpdate, ClientMessage, PlayerId, PlayerTransform, ServerMessage, WorldSnapshot,
    },
};

use crate::{
    mesh::Mesh,
    mesher::generate_mesh,
    player::{Player, PlayerController, RemotePlayer},
    uniform::UniformBinding,
    vertex::Vertex,
};
use wgpu::RenderPass;

pub struct Scene {
    pub model: glam::Mat4,
    pub chunk: Chunk,
    pub chunk_mesh: Mesh,
    pub uniform: UniformBinding,

    pub pipeline: wgpu::RenderPipeline,
    pub crosshair_pipeline: wgpu::RenderPipeline,
    pub highlight_pipeline: wgpu::RenderPipeline,

    pub target_block: Option<(glam::IVec3, glam::IVec3)>,
    pub highlight_mesh: Option<Mesh>,
    pub remote_player_mesh: Option<Mesh>,

    pub player: Player,
    pub player_controller: PlayerController,
}

impl Scene {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let chunk = Chunk::new();
        let (vertices, indices) = generate_mesh(&chunk);
        let chunk_mesh = Mesh::new(device, &vertices, &indices);
        let player = Player::new(glam::Vec3::new(8.0, 15.0, -15.0), glam::Quat::IDENTITY, 1.0);
        let player_controller = PlayerController::new(15.0, 0.2, 0.1);

        let uniform = UniformBinding::new(device);
        let pipeline = Self::create_pipeline(device, surface_format, &uniform);
        let crosshair_pipeline = Self::create_crosshair_pipeline(device, surface_format);

        let highlight_pipeline = Self::create_highlight_pipeline(device, surface_format, &uniform);
        Self {
            model: glam::Mat4::IDENTITY,
            chunk,
            uniform,
            pipeline,
            chunk_mesh,
            target_block: None,
            highlight_mesh: None,
            remote_player_mesh: None,
            highlight_pipeline,
            player,
            player_controller,
            crosshair_pipeline,
        }
    }

    pub fn interact(&mut self, device: &wgpu::Device, place_block: bool) -> Option<ClientMessage> {
        let origin = self.player.position;
        let forward = self.player.forward();

        // Raycast 10 units forward
        if let Some((hit_pos, hit_normal)) = self.chunk.raycast(origin, forward, 10.0) {
            if place_block {
                // Place a block adjacent to the face we hit
                let place_pos = hit_pos + hit_normal;
                self.set_block_and_remesh(device, place_pos, 3);
                Some(ClientMessage::PlaceBlock {
                    position: place_pos.to_array(),
                    block_type: 3,
                })
            } else {
                // Destroy the block we hit
                self.set_block_and_remesh(device, hit_pos, 0);
                Some(ClientMessage::BreakBlock {
                    position: hit_pos.to_array(),
                })
            }
        } else {
            None
        }
    }

    pub fn render<'rpass>(&'rpass self, renderpass: &mut RenderPass<'rpass>) {
        renderpass.set_pipeline(&self.pipeline);
        renderpass.set_bind_group(0, &self.uniform.bind_group, &[]);
        self.chunk_mesh.draw(renderpass);

        if let Some(remote_players) = &self.remote_player_mesh {
            remote_players.draw(renderpass);
        }

        // Draw the highlight if we are looking at a block
        if let Some(highlight) = &self.highlight_mesh {
            renderpass.set_pipeline(&self.highlight_pipeline);
            highlight.draw(renderpass);
        }

        renderpass.set_pipeline(&self.crosshair_pipeline);
        renderpass.draw(0..12, 0..1);
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        aspect_ratio: f32,
        delta_time: f32,
    ) -> Option<ClientMessage> {
        self.player.camera.aspect = aspect_ratio;
        self.player_controller
            .update_player(&mut self.player, delta_time);

        // Raycast
        let origin = self.player.position;
        let forward = self.player.forward();
        let new_target = self.chunk.raycast(origin, forward, 10.0);

        // Only rebuild the highlight mesh if what we are looking at changed!
        if new_target != self.target_block {
            self.target_block = new_target;
            self.highlight_mesh =
                new_target.map(|(pos, normal)| Self::build_highlight_mesh(device, pos, normal));
        }

        self.uniform.update_buffer(
            queue,
            0,
            super::uniform::UniformBuffer {
                mvp: self.player.camera.build_view_projection_matrix() * self.model,
            },
        );

        if self.player.player_id.is_some() && self.player.should_send_move() {
            return Some(ClientMessage::MovePlayer {
                position: self.player.position.to_array(),
                rotation: quat_to_array(self.player.rotation),
            });
        }

        None
    }

    pub fn apply_server_message(
        &mut self,
        device: &wgpu::Device,
        message: ServerMessage,
    ) -> Result<(), String> {
        match message {
            ServerMessage::Welcome {
                player_id,
                snapshot,
                ..
            } => {
                self.player.player_id = Some(player_id);
                self.apply_snapshot(device, player_id, snapshot)?;
            }
            ServerMessage::WorldUpdate {
                players,
                blocks,
                disconnected_players,
                ..
            } => {
                let mut remote_players_changed = false;
                for player in players {
                    if self.apply_player_transform(player) {
                        remote_players_changed = true;
                    }
                }
                for player_id in disconnected_players {
                    if self.player.remote_players.remove(&player_id).is_some() {
                        remote_players_changed = true;
                    }
                }
                if remote_players_changed {
                    self.rebuild_remote_player_mesh(device);
                }

                if apply_block_updates(&mut self.chunk, &blocks) {
                    self.remesh_chunk(device);
                }
            }
            ServerMessage::Error { message } => {
                return Err(message);
            }
        }

        Ok(())
    }

    fn apply_snapshot(
        &mut self,
        device: &wgpu::Device,
        player_id: PlayerId,
        snapshot: WorldSnapshot,
    ) -> Result<(), String> {
        if snapshot.chunk_blocks.len() != self.chunk.blocks.len() {
            return Err(format!(
                "server sent {} chunk blocks, expected {}",
                snapshot.chunk_blocks.len(),
                self.chunk.blocks.len()
            ));
        }

        for (index, block) in snapshot.chunk_blocks.into_iter().enumerate() {
            self.chunk.blocks[index] = block;
        }
        self.remesh_chunk(device);

        self.player.remote_players.clear();
        for transform in snapshot.players {
            if transform.player_id == player_id {
                self.player.set_transform(
                    glam::Vec3::from_array(transform.position),
                    quat_from_array(transform.rotation),
                );
            } else {
                self.player.remote_players.insert(
                    transform.player_id,
                    RemotePlayer {
                        position: glam::Vec3::from_array(transform.position),
                        rotation: quat_from_array(transform.rotation),
                    },
                );
            }
        }
        self.rebuild_remote_player_mesh(device);

        Ok(())
    }

    fn apply_player_transform(&mut self, transform: PlayerTransform) -> bool {
        if Some(transform.player_id) == self.player.player_id {
            return false;
        }

        let remote = RemotePlayer {
            position: glam::Vec3::from_array(transform.position),
            rotation: quat_from_array(transform.rotation),
        };

        if self.player.remote_players.get(&transform.player_id) == Some(&remote) {
            return false;
        }

        self.player
            .remote_players
            .insert(transform.player_id, remote);
        true
    }

    fn set_block_and_remesh(
        &mut self,
        device: &wgpu::Device,
        position: glam::IVec3,
        block_type: u8,
    ) {
        if self.chunk.get_block(position) == block_type {
            return;
        }

        self.chunk.set_block(position, block_type);
        self.remesh_chunk(device);
    }

    fn remesh_chunk(&mut self, device: &wgpu::Device) {
        let (vertices, indices) = generate_mesh(&self.chunk);
        self.chunk_mesh = Mesh::new(device, &vertices, &indices);
    }

    fn rebuild_remote_player_mesh(&mut self, device: &wgpu::Device) {
        let (vertices, indices) = build_remote_player_mesh(&self.player.remote_players);
        self.remote_player_mesh =
            (!indices.is_empty()).then(|| Mesh::new(device, &vertices, &indices));
    }

    fn create_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        uniform: &UniformBinding,
    ) -> wgpu::RenderPipeline {
        let shader_module = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&uniform.bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vertex_main"),
                buffers: &[Vertex::description(&Vertex::vertex_attributes())],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::renderer::Renderer::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        })
    }

    fn create_crosshair_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader_module = device.create_shader_module(wgpu::include_wgsl!("crosshair.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Crosshair Pipeline Layout"),
            bind_group_layouts: &[], // No uniforms needed!
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Crosshair Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[], // Bufferless!
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::renderer::Renderer::DEPTH_FORMAT,
                depth_write_enabled: false, // Don't write to the depth buffer
                depth_compare: wgpu::CompareFunction::Always, // Always draw on top of everything
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        })
    }

    fn create_highlight_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        uniform: &UniformBinding,
    ) -> wgpu::RenderPipeline {
        let shader_module = device.create_shader_module(wgpu::include_wgsl!("highlight.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&uniform.bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Highlight Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::description(&Vertex::vertex_attributes())],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling needed for a flat plane
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::renderer::Renderer::DEPTH_FORMAT,
                depth_write_enabled: false, // Don't block other things from drawing
                depth_compare: wgpu::CompareFunction::LessEqual, // Allow it to draw directly on the surface
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING), // Turn on transparency!
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        })
    }

    fn build_highlight_mesh(device: &wgpu::Device, pos: glam::IVec3, normal: glam::IVec3) -> Mesh {
        // Push the highlight outward slightly so it doesn't Z-fight with the block
        let offset = 0.005;
        let offset_vec = glam::Vec3::new(
            normal.x as f32 * offset,
            normal.y as f32 * offset,
            normal.z as f32 * offset,
        );
        let base_pos = glam::Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32) + offset_vec;

        // Grab the 4 local corners based on which face we hit
        let local_positions = match (normal.x, normal.y, normal.z) {
            (0, 1, 0) => [
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
            (0, -1, 0) => [
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
            ],
            (1, 0, 0) => [
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
            ],
            (-1, 0, 0) => [
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            (0, 0, 1) => [
                [0.0, 0.0, 1.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 0.0, 1.0],
            ],
            (0, 0, -1) => [
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
            ],
            _ => [[0.0; 3]; 4],
        };

        let mut vertices = Vec::new();
        for p in local_positions {
            vertices.push(Vertex::new(
                [base_pos.x + p[0], base_pos.y + p[1], base_pos.z + p[2]],
                [0.0, 0.0, 0.0], // Normal isn't used by the highlight shader
                [1.0, 1.0, 1.0], // Color is overridden by the highlight shader
            ));
        }

        let indices = vec![0, 1, 2, 0, 2, 3];
        Mesh::new(device, &vertices, &indices)
    }
}

fn apply_block_updates(chunk: &mut Chunk, blocks: &[BlockUpdate]) -> bool {
    let mut changed = false;

    for block in blocks {
        let position = glam::IVec3::from_array(block.position);
        if chunk.get_block(position) == block.block_type {
            continue;
        }

        chunk.set_block(position, block.block_type);
        changed = true;
    }

    changed
}

fn build_remote_player_mesh(
    remote_players: &std::collections::HashMap<PlayerId, RemotePlayer>,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for remote_player in remote_players.values() {
        add_box(
            &mut vertices,
            &mut indices,
            remote_player.position - glam::Vec3::splat(0.08),
            remote_player.position + glam::Vec3::splat(0.08),
            [1.0, 0.0, 0.0],
        );

        let forward = (remote_player.rotation * glam::Vec3::Z).normalize_or_zero();
        let start = remote_player.position + glam::Vec3::Y * 0.02;
        let end = start + forward * 0.75;
        add_oriented_box(
            &mut vertices,
            &mut indices,
            start,
            end,
            0.025,
            [1.0, 0.9, 0.0],
        );
    }

    (vertices, indices)
}

fn add_oriented_box(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    start: glam::Vec3,
    end: glam::Vec3,
    half_width: f32,
    color: [f32; 3],
) {
    let forward = (end - start).normalize_or_zero();
    if forward == glam::Vec3::ZERO {
        return;
    }

    let right = forward.cross(glam::Vec3::Y).normalize_or_zero();
    let right = if right == glam::Vec3::ZERO {
        glam::Vec3::X
    } else {
        right
    };
    let up = right.cross(forward).normalize_or_zero();

    let corners = [
        start - right * half_width - up * half_width,
        start + right * half_width - up * half_width,
        start + right * half_width + up * half_width,
        start - right * half_width + up * half_width,
        end - right * half_width - up * half_width,
        end + right * half_width - up * half_width,
        end + right * half_width + up * half_width,
        end - right * half_width + up * half_width,
    ];

    add_box_from_corners(vertices, indices, corners, color);
}

fn add_box(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    min: glam::Vec3,
    max: glam::Vec3,
    color: [f32; 3],
) {
    let corners = [
        glam::Vec3::new(min.x, min.y, min.z),
        glam::Vec3::new(max.x, min.y, min.z),
        glam::Vec3::new(max.x, max.y, min.z),
        glam::Vec3::new(min.x, max.y, min.z),
        glam::Vec3::new(min.x, min.y, max.z),
        glam::Vec3::new(max.x, min.y, max.z),
        glam::Vec3::new(max.x, max.y, max.z),
        glam::Vec3::new(min.x, max.y, max.z),
    ];

    add_box_from_corners(vertices, indices, corners, color);
}

fn add_box_from_corners(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    corners: [glam::Vec3; 8],
    color: [f32; 3],
) {
    let faces = [
        ([0, 1, 2, 3], glam::Vec3::NEG_Z),
        ([5, 4, 7, 6], glam::Vec3::Z),
        ([4, 0, 3, 7], glam::Vec3::NEG_X),
        ([1, 5, 6, 2], glam::Vec3::X),
        ([3, 2, 6, 7], glam::Vec3::Y),
        ([4, 5, 1, 0], glam::Vec3::NEG_Y),
    ];

    for (face, normal) in faces {
        let base = vertices.len() as u32;
        for corner_index in face {
            let corner = corners[corner_index];
            vertices.push(Vertex::new(corner.to_array(), normal.to_array(), color));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn quat_to_array(rotation: glam::Quat) -> [f32; 4] {
    [rotation.x, rotation.y, rotation.z, rotation.w]
}

fn quat_from_array(rotation: [f32; 4]) -> glam::Quat {
    glam::Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_updates_report_unchanged_chunk_without_dirtying_mesh() {
        let mut chunk = Chunk::empty();
        chunk.set_block(glam::IVec3::new(1, 2, 3), 4);

        let changed = apply_block_updates(
            &mut chunk,
            &[BlockUpdate {
                position: [1, 2, 3],
                block_type: 4,
            }],
        );

        assert!(!changed);
    }

    #[test]
    fn block_updates_report_changed_chunk() {
        let mut chunk = Chunk::empty();

        let changed = apply_block_updates(
            &mut chunk,
            &[BlockUpdate {
                position: [1, 2, 3],
                block_type: 4,
            }],
        );

        assert!(changed);
        assert_eq!(chunk.get_block(glam::IVec3::new(1, 2, 3)), 4);
    }

    #[test]
    fn remote_player_mesh_is_empty_without_remote_players() {
        let players = std::collections::HashMap::new();
        let (_vertices, indices) = build_remote_player_mesh(&players);

        assert!(indices.is_empty());
    }
}
