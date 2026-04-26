use std::collections::HashMap;

use shared::protocol::PlayerId;
use wgpu::{
    CommandEncoderDescriptor, LoadOp, Operations, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp,
};

use crate::{
    gpu::Gpu,
    mesh::Mesh,
    mesher::generate_mesh,
    player::RemotePlayer,
    uniform::{UniformBinding, UniformBuffer},
    vertex::Vertex,
    world::World,
};

pub struct Renderer {
    pub gpu: Gpu,
    pub depth_texture_view: wgpu::TextureView,
    pipelines: Pipelines,
    uniform: UniformBinding,
    chunk_mesh: Option<Mesh>,
    remote_player_mesh: Option<Mesh>,
    highlight_mesh: Option<Mesh>,
}

struct Pipelines {
    chunk: wgpu::RenderPipeline,
    highlight: wgpu::RenderPipeline,
    crosshair: wgpu::RenderPipeline,
}

impl Renderer {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        let gpu = Gpu::new_async(window, width, height).await;
        let depth_texture_view = gpu.create_depth_texture(width, height);

        let uniform = UniformBinding::new(&gpu.device);
        let pipelines = Pipelines {
            chunk: create_pipeline(&gpu.device, gpu.surface_format, &uniform),
            highlight: create_highlight_pipeline(&gpu.device, gpu.surface_format, &uniform),
            crosshair: create_crosshair_pipeline(&gpu.device, gpu.surface_format),
        };

        Self {
            gpu,
            depth_texture_view,
            pipelines,
            uniform,
            chunk_mesh: None,
            remote_player_mesh: None,
            highlight_mesh: None,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
        self.depth_texture_view = self.gpu.create_depth_texture(width, height);
    }

    pub fn sync(&mut self, world: &mut World) {
        if world.dirty.chunk {
            let (vertices, indices) = generate_mesh(&world.chunk);
            self.chunk_mesh = Some(Mesh::new(&self.gpu.device, &vertices, &indices));
        }

        if world.dirty.remote_players {
            let (vertices, indices) = build_remote_player_mesh(&world.remote_players);
            self.remote_player_mesh =
                (!indices.is_empty()).then(|| Mesh::new(&self.gpu.device, &vertices, &indices));
        }

        if world.dirty.highlight {
            self.highlight_mesh = world
                .target
                .map(|(pos, normal)| build_highlight_mesh(&self.gpu.device, pos, normal));
        }

        world.dirty.clear();
    }

    pub fn render(&mut self, world: &World) {
        self.uniform.update_buffer(
            &self.gpu.queue,
            0,
            UniformBuffer {
                mvp: world.player.camera.build_view_projection_matrix(),
            },
        );

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let surface_texture = self.next_surface_texture();

        let surface_texture_view =
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: wgpu::Label::default(),
                    aspect: wgpu::TextureAspect::default(),
                    format: Some(self.gpu.surface_format),
                    dimension: None,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                    usage: None,
                });

        encoder.insert_debug_marker("Render world");

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &surface_texture_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color {
                            r: 0.47,
                            g: 0.65,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipelines.chunk);
            render_pass.set_bind_group(0, &self.uniform.bind_group, &[]);
            if let Some(chunk_mesh) = &self.chunk_mesh {
                chunk_mesh.draw(&mut render_pass);
            }

            if let Some(remote_players) = &self.remote_player_mesh {
                remote_players.draw(&mut render_pass);
            }

            if let Some(highlight) = &self.highlight_mesh {
                render_pass.set_pipeline(&self.pipelines.highlight);
                highlight.draw(&mut render_pass);
            }

            render_pass.set_pipeline(&self.pipelines.crosshair);
            render_pass.draw(0..12, 0..1);
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    }

    fn next_surface_texture(&self) -> wgpu::SurfaceTexture {
        match self.gpu.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Outdated) => {
                self.gpu
                    .surface
                    .configure(&self.gpu.device, &self.gpu.surface_config);
                self.gpu
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture after reconfiguration!")
            }
            Err(error) => panic!("Failed to get surface texture: {:?}", error),
        }
    }
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
            format: Renderer::DEPTH_FORMAT,
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
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Crosshair Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("vs_main"),
            buffers: &[],
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
            format: Renderer::DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always,
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
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
            unclipped_depth: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: Renderer::DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
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

fn build_highlight_mesh(device: &wgpu::Device, pos: glam::IVec3, normal: glam::IVec3) -> Mesh {
    let offset = 0.005;
    let offset_vec = glam::Vec3::new(
        normal.x as f32 * offset,
        normal.y as f32 * offset,
        normal.z as f32 * offset,
    );
    let base_pos = glam::Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32) + offset_vec;

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
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        ));
    }

    Mesh::new(device, &vertices, &[0, 1, 2, 0, 2, 3])
}

fn build_remote_player_mesh(
    remote_players: &HashMap<PlayerId, RemotePlayer>,
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
