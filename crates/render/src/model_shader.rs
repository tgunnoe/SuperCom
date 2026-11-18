//! The WGSL source of the model pipelines, a second module apart from the
//! terrain and water module of `shader.rs`.

use crate::{AMBIENT, LIGHT_DIRECTION};

/// The shader with `$` markers where the Rust constants go; WGSL has no
/// `$`, so a marker left unreplaced fails to compile rather than shading
/// wrong.
const TEMPLATE: &str = "
struct Frame {
    view_projection: mat4x4<f32>,
    range: vec4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;

const LIGHT: vec3<f32> = $LIGHT;
const AMBIENT: f32 = $AMBIENT;

struct ModelVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct Placed {
    @location(2) column_0: vec4<f32>,
    @location(3) column_1: vec4<f32>,
    @location(4) column_2: vec4<f32>,
    @location(5) column_3: vec4<f32>,
    @location(6) color: vec3<f32>,
}

struct LitFragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
}

struct FlatFragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
}

fn world(placed: Placed, position: vec3<f32>) -> vec4<f32> {
    let transform = mat4x4<f32>(placed.column_0, placed.column_1, placed.column_2, placed.column_3);
    return frame.view_projection * (transform * vec4<f32>(position, 1.0));
}

@vertex
fn vs_lit(vertex: ModelVertex, placed: Placed) -> LitFragment {
    var out: LitFragment;
    out.clip = world(placed, vertex.position);
    let turn = mat3x3<f32>(placed.column_0.xyz, placed.column_1.xyz, placed.column_2.xyz);
    out.normal = normalize(turn * vertex.normal);
    out.color = placed.color;
    return out;
}

@fragment
fn fs_lit(fragment: LitFragment) -> @location(0) vec4<f32> {
    let facing = max(dot(normalize(fragment.normal), LIGHT), 0.0);
    let light = AMBIENT + (1.0 - AMBIENT) * facing;
    return vec4<f32>(clamp(fragment.color * light, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

@vertex
fn vs_flat(vertex: ModelVertex, placed: Placed) -> FlatFragment {
    var out: FlatFragment;
    out.clip = world(placed, vertex.position);
    out.color = placed.color;
    return out;
}

@fragment
fn fs_flat(fragment: FlatFragment) -> @location(0) vec4<f32> {
    return vec4<f32>(fragment.color, 1.0);
}
";

/// The WGSL source of the lit and flat model pipelines, with the light
/// direction and the ambient share written in from the crate's Rust
/// constants. Its entry points are `vs_lit`, `fs_lit`, `vs_flat`, and
/// `fs_flat`.
#[must_use]
pub fn model_shader_source() -> String {
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
