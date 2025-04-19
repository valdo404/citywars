struct Uniforms {
    mvp: mat4x4<f32>;
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>;
    @location(1) normal: vec3<f32>;
};

struct VertexOutput {
    @builtin(position) Position: vec4<f32>;
    @location(0) v_normal: vec3<f32>;
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.Position = uniforms.mvp * vec4<f32>(input.position, 1.0);
    out.v_normal = input.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let brightness = max(dot(in.v_normal, light), 0.0);
    let color = vec3<f32>(0.2, 0.5, 1.0) * brightness + vec3<f32>(0.1);
    return vec4<f32>(color, 1.0);
}
