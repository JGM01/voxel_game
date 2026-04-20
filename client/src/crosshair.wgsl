@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {
    // Normalized Device Coordinates (NDC) go from -1.0 to 1.0, so 0.0 is the exact center.
    var pos = array<vec2<f32>, 12>(
        // Horizontal bar
        vec2<f32>(-0.02, -0.002), vec2<f32>(0.02, -0.002), vec2<f32>(-0.02, 0.002),
        vec2<f32>(-0.02, 0.002),  vec2<f32>(0.02, -0.002), vec2<f32>(0.02, 0.002),
        // Vertical bar
        vec2<f32>(-0.002, -0.03), vec2<f32>(0.002, -0.03), vec2<f32>(-0.002, 0.03),
        vec2<f32>(-0.002, 0.03),  vec2<f32>(0.002, -0.03), vec2<f32>(0.002, 0.03)
    );
    
    // Z is 0.0, W is 1.0
    return vec4<f32>(pos[idx], 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // White color with slight transparency
    return vec4<f32>(1.0, 1.0, 1.0, 0.8); 
}
