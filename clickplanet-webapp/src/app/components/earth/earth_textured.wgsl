// Textured Earth Shader (WGSL)
const PI: f32 = 3.141592653589793;

struct Uniforms {
    mvp: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var earth_texture: texture_2d<f32>;

@group(0) @binding(2)
var earth_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) Position: vec4<f32>,
    @location(0) v_uv: vec2<f32>,
    @location(1) v_normal: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.Position = uniforms.mvp * vec4<f32>(input.position, 1.0);
    // spherical UV calculation
    let theta = acos(input.position.y);
    let phi = atan2(input.position.z, input.position.x);
    let u = phi / (2.0 * PI) + 0.5;
    let v = theta / PI;
    out.v_uv = vec2<f32>(u, v);
    out.v_normal = input.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(earth_texture, earth_sampler, in.v_uv);
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let brightness = max(dot(in.v_normal, light_dir), 0.0);
    return color * vec4<f32>(brightness, brightness, brightness, 1.0);
}
