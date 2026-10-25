// Tiles: one camera-facing disc per tile, drawn with instancing (4-vertex strip per instance).
// Reproduces the original point shader: unowned tiles are white (alpha 0.3, 0.6 when hovered),
// owned tiles show the owner's flag from the atlas, cropped to a square (alpha 0.7, 1.0 hovered).

struct Frame {
    view_proj: mat4x4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    params: vec4<f32>,
    hovered: vec4<u32>,
};

struct Regions {
    // atlas region (x, y, width, height) in pixels for each country index; index 0 = no owner
    r: array<vec4<f32>, 512>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<uniform> regions: Regions;
@group(0) @binding(3) var atlas_texture: texture_2d<f32>;
@group(0) @binding(4) var linear_sampler: sampler;

struct TileOutput {
    @builtin(position) position: vec4<f32>,
    // (0, 0) top-left of the disc, (1, 1) bottom-right
    @location(0) quad: vec2<f32>,
    @location(1) @interpolate(flat) region: vec4<f32>,
    @location(2) @interpolate(flat) hover: f32,
};

@vertex
fn vs_tile(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
    @location(0) center: vec3<f32>,
    @location(1) owner: u32,
) -> TileOutput {
    var out: TileOutput;
    let corner = vec2<f32>(f32(vertex_index & 1u), f32((vertex_index >> 1u) & 1u));
    let offset = (corner - vec2<f32>(0.5, 0.5)) * frame.params.x;
    let world = center + frame.right.xyz * offset.x + frame.up.xyz * offset.y;
    out.position = frame.view_proj * vec4<f32>(world, 1.0);
    out.quad = vec2<f32>(corner.x, 1.0 - corner.y);
    out.region = regions.r[min(owner, 511u)];
    out.hover = select(0.0, 1.0, instance_index + 1u == frame.hovered.x);
    return out;
}

@fragment
fn fs_tile(in: TileOutput) -> @location(0) vec4<f32> {
    let atlas_size = frame.params.yz;
    // Sample before any non-uniform control flow.
    var uv = in.quad;
    let aspect = in.region.z / max(in.region.w, 1.0);
    if (aspect > 1.0) {
        uv.x = (uv.x - 0.5) / aspect + 0.5;
    } else {
        uv.y = (uv.y - 0.5) * aspect + 0.5;
    }
    let atlas_uv = (in.region.xy + uv * in.region.zw) / atlas_size;
    let flag = textureSampleLevel(atlas_texture, linear_sampler, atlas_uv, 0.0);

    if (length(in.quad - vec2<f32>(0.5, 0.5)) > 0.5) {
        discard;
    }
    let hovered = in.hover > 0.5;
    if (in.region.z == 0.0 || in.region.w == 0.0) {
        return vec4<f32>(1.0, 1.0, 1.0, select(0.3, 0.6, hovered));
    }
    return vec4<f32>(flag.rgb, select(0.7, 1.0, hovered));
}
