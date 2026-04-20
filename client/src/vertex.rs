#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 4],
    normal: [f32; 4],
    color: [f32; 4],
}

impl Vertex {
    pub fn new(pos: [f32; 3], normal: [f32; 3], color: [f32; 3]) -> Self {
        Self {
            position: [pos[0], pos[1], pos[2], 1.0],
            normal: [normal[0], normal[1], normal[2], 0.0],
            color: [color[0], color[1], color[2], 1.0],
        }
    }

    pub fn vertex_attributes() -> Vec<wgpu::VertexAttribute> {
        wgpu::vertex_attr_array![
            0 => Float32x4, // Position
            1 => Float32x4, // Normal
            2 => Float32x4  // Color
        ]
        .to_vec()
    }

    pub fn description(attributes: &[wgpu::VertexAttribute]) -> wgpu::VertexBufferLayout<'_> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes,
        }
    }
}
