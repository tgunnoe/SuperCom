//! The WGSL source and the colours the renderer states.

use crate::{AMBIENT, LIGHT_DIRECTION, RAMP};

/// The linear RGB colour of a pixel no geometry covers.
pub const CLEAR_COLOR: [f32; 3] = [0.01, 0.01, 0.02];

/// The linear RGB colour of the water plane, opaque. Its blue channel
/// exceeds its red and green, which no terrain colour's does.
pub const WATER_COLOR: [f32; 3] = [0.04, 0.22, 0.45];

/// The shader with `$` markers where the Rust constants go. WGSL has no `$`,
/// so a marker left unreplaced fails to compile rather than shading wrong.
const TEMPLATE: &str = "
struct Frame {
    view_projection: mat4x4<f32>,
    range: vec4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;

const LIGHT: vec3<f32> = $LIGHT;
const AMBIENT: f32 = $AMBIENT;
const STOP_0: vec3<f32> = $STOP_0;
const STOP_1: vec3<f32> = $STOP_1;
const STOP_2: vec3<f32> = $STOP_2;
const WATER: vec3<f32> = $WATER;

struct TerrainVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct TerrainFragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) elevation: f32,
}

@vertex
fn vs_terrain(vertex: TerrainVertex) -> TerrainFragment {
    var out: TerrainFragment;
    out.clip = frame.view_projection * vec4<f32>(vertex.position, 1.0);
    out.normal = vertex.normal;
    out.elevation = vertex.position.y;
    return out;
}

@fragment
fn fs_terrain(fragment: TerrainFragment) -> @location(0) vec4<f32> {
    let span = frame.range.y - frame.range.x;
    var height = 0.0;
    if span > 0.0 {
        height = (fragment.elevation - frame.range.x) / span;
    }
    let t = clamp(height, 0.0, 1.0) * 2.0;
    var from_stop = STOP_0;
    var to_stop = STOP_1;
    var along = t;
    if t >= 1.0 {
        from_stop = STOP_1;
        to_stop = STOP_2;
        along = t - 1.0;
    }
    let facing = max(dot(normalize(fragment.normal), LIGHT), 0.0);
    let light = AMBIENT + (1.0 - AMBIENT) * facing;
    let color = (from_stop + (to_stop - from_stop) * along) * light;
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

@vertex
fn vs_water(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return frame.view_projection * vec4<f32>(position, 1.0);
}

@fragment
fn fs_water() -> @location(0) vec4<f32> {
    return vec4<f32>(WATER, 1.0);
}
";

/// A WGSL `vec3<f32>` constructor for `v`, each component as the shortest
/// literal that reads back as the same `f32`.
fn vec3(v: [f32; 3]) -> String {
    format!("vec3<f32>({:?}, {:?}, {:?})", v[0], v[1], v[2])
}

/// The WGSL source of the terrain and water pipelines, with the light
/// direction, the ambient share, the ramp stops, and the water colour
/// written in from the crate's Rust constants.
#[must_use]
pub fn shader_source() -> String {
    TEMPLATE
        .replace("$LIGHT", &vec3(LIGHT_DIRECTION))
        .replace("$AMBIENT", &format!("{AMBIENT:?}"))
        .replace("$STOP_0", &vec3(RAMP[0]))
        .replace("$STOP_1", &vec3(RAMP[1]))
        .replace("$STOP_2", &vec3(RAMP[2]))
        .replace("$WATER", &vec3(WATER_COLOR))
}
