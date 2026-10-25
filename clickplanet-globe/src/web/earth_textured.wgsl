// Textured Earth sphere. The texture is equirectangular and mapped like three.js'
// IcosahedronGeometry UVs used by the original front end and by coordinates.json:
// u = atan2(z, -x) / 2pi + 0.5, v (from the top of the image) = 0.5 - asin(y) / pi.
// Lighting is a flat ambient light, like the original (AmbientLight only).
const PI: f32 = 3.141592653589793;

struct Frame {
    view_proj: mat4x4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    // x: tile diameter (world units), y/z: atlas size in pixels
    params: vec4<f32>,
    // x: hovered tile index + 1 (0 = none)
    hovered: vec4<u32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(2) var earth_texture: texture_2d<f32>;
@group(0) @binding(4) var linear_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) direction: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = frame.view_proj * vec4<f32>(input.position, 1.0);
    out.direction = input.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = normalize(in.direction);
    let u = atan2(d.z, -d.x) / (2.0 * PI) + 0.5;
    let v = 0.5 - asin(clamp(d.y, -1.0, 1.0)) / PI;
    // Avoid the mip seam where u wraps from 1 to 0: use the derivatives of whichever of u and
    // u shifted by half a turn is continuous at this pixel.
    let u_shifted = fract(u + 0.5) - 0.5;
    let use_shifted = fwidth(u_shifted) < fwidth(u);
    let du_dx = select(dpdx(u), dpdx(u_shifted), use_shifted);
    let du_dy = select(dpdy(u), dpdy(u_shifted), use_shifted);
    let uv = vec2<f32>(u, v);
    let color = textureSampleGrad(earth_texture, linear_sampler, uv,
        vec2<f32>(du_dx, dpdx(v)), vec2<f32>(du_dy, dpdy(v)));
    return vec4<f32>(color.rgb, 1.0);
}
