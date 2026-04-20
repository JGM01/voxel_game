struct Uniform {
    mvp: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> ubo: Uniform;

struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(vert: VertexInput) -> @builtin(position) vec4<f32> {
    return ubo.mvp * vert.position;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // 1.0 for RGB (White), and 0.4 for Alpha (40% opacity)
    return vec4<f32>(1.0, 1.0, 1.0, 0.4); 
}
