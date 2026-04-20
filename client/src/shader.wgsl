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

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vertex_main(vert: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.color = vert.color;
    out.normal = vert.normal.xyz; 
    out.position = ubo.mvp * vert.position;
    return out;
};

@fragment
fn fragment_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let ambient_strength = 0.3;
    let ambient_color = in.color.rgb * ambient_strength;
    
    let diffuse_strength = max(dot(in.normal, light_dir), 0.0);
    let diffuse_color = in.color.rgb * diffuse_strength;
    
    let final_color = ambient_color + diffuse_color;
    
    return vec4<f32>(final_color, in.color.a);
}
