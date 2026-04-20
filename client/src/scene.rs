use shared::chunk::Chunk;

use crate::{
    camera::{Camera, CameraController},
    mesh::Mesh,
    mesher::generate_mesh,
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

    pub camera: Camera,
    pub camera_controller: CameraController,
}

impl Scene {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let chunk = Chunk::new();
        let (vertices, indices) = generate_mesh(&chunk);
        let chunk_mesh = Mesh::new(device, &vertices, &indices);
        let camera = Camera {
            position: glam::Vec3::new(8.0, 15.0, -15.0),
            rotation: glam::Quat::IDENTITY,
            aspect: 1.0,
            fov: 80.0, // Assumes degrees based on the new `to_radians()` in Camera
            near: 0.1,
            far: 1000.0,
        };

        let camera_controller = CameraController::new(15.0, 0.2, 0.1);

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
            highlight_pipeline,
            camera,
            camera_controller,
            crosshair_pipeline,
        }
    }

    pub fn interact(&mut self, device: &wgpu::Device, place_block: bool) {
        let origin = self.camera.position;
        let forward = self.camera.forward();

        // Raycast 10 units forward
        if let Some((hit_pos, hit_normal)) = self.chunk.raycast(origin, forward, 10.0) {
            if place_block {
                // Place a block adjacent to the face we hit
                let place_pos = hit_pos + hit_normal;
                self.chunk
                    .set_block(place_pos.x, place_pos.y, place_pos.z, 3); // Place a stone block
            } else {
                // Destroy the block we hit
                self.chunk.set_block(hit_pos.x, hit_pos.y, hit_pos.z, 0);
            }

            // Re-run the CPU mesher
            let (vertices, indices) = generate_mesh(&self.chunk);

            // Overwrite the old GPU buffers with the new mesh
            self.chunk_mesh = Mesh::new(device, &vertices, &indices);
        }
    }

    pub fn render<'rpass>(&'rpass self, renderpass: &mut RenderPass<'rpass>) {
        renderpass.set_pipeline(&self.pipeline);
        renderpass.set_bind_group(0, &self.uniform.bind_group, &[]);
        self.chunk_mesh.draw(renderpass);

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
    ) {
        self.camera.aspect = aspect_ratio;
        self.camera_controller
            .update_camera(&mut self.camera, delta_time);

        // Raycast
        let origin = self.camera.position;
        let forward = self.camera.forward();
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
                mvp: self.camera.build_view_projection_matrix() * self.model,
            },
        );
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
