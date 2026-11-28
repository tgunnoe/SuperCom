//! The WGSL source of the textured terrain, a third module apart from the
//! first light module of `shader.rs` and the models' of `model_shader.rs`.

use crate::{AMBIENT, LIGHT_DIRECTION};

/// The shader with `$` markers where the Rust constants go; WGSL has no
/// `$`, so a marker left unreplaced fails to compile rather than shading
/// wrong. Group 1 holds the textured terrain's uniform, its two samplers,
/// the ten layers (bindings 3 to 12), and the two masks (13 and 14). Every
/// layer and mask is sampled in uniform control flow and an unnamed layer
/// weighs nothing by its flag, so no lookup sits behind a branch.
const TEMPLATE: &str = "
struct Frame {
    view_projection: mat4x4<f32>,
    range: vec4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;

struct Textured {
    extent: vec4<f32>,
    scales: array<vec4<f32>, 3>,
    named: array<vec4<f32>, 3>,
}
@group(1) @binding(0) var<uniform> textured: Textured;
@group(1) @binding(1) var mask_sampler: sampler;
@group(1) @binding(2) var layer_sampler: sampler;
@group(1) @binding(3) var layer_0: texture_2d<f32>;
@group(1) @binding(4) var layer_1: texture_2d<f32>;
@group(1) @binding(5) var layer_2: texture_2d<f32>;
@group(1) @binding(6) var layer_3: texture_2d<f32>;
@group(1) @binding(7) var layer_4: texture_2d<f32>;
@group(1) @binding(8) var layer_5: texture_2d<f32>;
@group(1) @binding(9) var layer_6: texture_2d<f32>;
@group(1) @binding(10) var layer_7: texture_2d<f32>;
@group(1) @binding(11) var layer_8: texture_2d<f32>;
@group(1) @binding(12) var layer_9: texture_2d<f32>;
@group(1) @binding(13) var mask_0: texture_2d<f32>;
@group(1) @binding(14) var mask_1: texture_2d<f32>;

const LIGHT: vec3<f32> = $LIGHT;
const AMBIENT: f32 = $AMBIENT;

struct TerrainVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct TexturedFragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec2<f32>,
}

@vertex
fn vs_textured(vertex: TerrainVertex) -> TexturedFragment {
    var out: TexturedFragment;
    out.clip = frame.view_projection * vec4<f32>(vertex.position, 1.0);
    out.normal = vertex.normal;
    out.world = vertex.position.xz;
    return out;
}

fn weight(m: f32) -> f32 {
    return clamp(2.0 * m - 1.0, 0.0, 1.0);
}

fn named_of(slot: u32) -> f32 {
    return textured.named[slot / 4u][slot % 4u];
}

fn layer_at(t: texture_2d<f32>, slot: u32, world: vec2<f32>) -> vec4<f32> {
    return textureSample(t, layer_sampler, world / textured.scales[slot / 4u][slot % 4u]);
}

fn lay(under: vec3<f32>, t: texture_2d<f32>, slot: u32, world: vec2<f32>, m: f32) -> vec3<f32> {
    return mix(under, layer_at(t, slot, world).rgb, weight(m) * named_of(slot));
}

fn srgb_decode(v: vec3<f32>) -> vec3<f32> {
    let low = v / 12.92;
    let high = pow((v + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, v <= vec3<f32>(0.04045));
}

@fragment
fn fs_textured(fragment: TexturedFragment) -> @location(0) vec4<f32> {
    let uv = clamp(fragment.world / textured.extent.xy, vec2<f32>(0.0), vec2<f32>(1.0));
    let first = textureSample(mask_0, mask_sampler, uv);
    let second = textureSample(mask_1, mask_sampler, uv);
    let w = fragment.world;
    var color = layer_at(layer_0, 0u, w).rgb;
    color = lay(color, layer_1, 1u, w, first.x);
    color = lay(color, layer_2, 2u, w, first.y);
    color = lay(color, layer_3, 3u, w, first.z);
    color = lay(color, layer_4, 4u, w, first.w);
    color = lay(color, layer_5, 5u, w, second.x);
    color = lay(color, layer_6, 6u, w, second.y);
    color = lay(color, layer_7, 7u, w, second.z);
    color = lay(color, layer_8, 8u, w, second.w);
    let top = layer_at(layer_9, 9u, w);
    color = mix(color, top.rgb, top.a);
    let facing = max(dot(normalize(fragment.normal), LIGHT), 0.0);
    let light = AMBIENT + (1.0 - AMBIENT) * facing;
    let lit = clamp(color * light, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(clamp(srgb_decode(lit), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
";

/// The WGSL source of the textured terrain pipeline, with the light
/// direction and the ambient share written in from the crate's Rust
/// constants. Its entry points are `vs_textured` and `fs_textured`.
#[must_use]
pub fn textured_shader_source() -> String {
    TEMPLATE
        .replace(
            "$LIGHT",
            &format!(
                "vec3<f32>({:?}, {:?}, {:?})",
                LIGHT_DIRECTION[0], LIGHT_DIRECTION[1], LIGHT_DIRECTION[2]
            ),
        )
        .replace("$AMBIENT", &format!("{AMBIENT:?}"))
}
