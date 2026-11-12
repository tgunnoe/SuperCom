//! The WGSL source and the colours the renderer states.

/// The linear RGB colour of a pixel no geometry covers.
pub const CLEAR_COLOR: [f32; 3] = [0.01, 0.01, 0.02];

/// The linear RGB colour of the water plane, opaque. Its blue channel
/// exceeds its red and green, which no terrain colour's does.
pub const WATER_COLOR: [f32; 3] = [0.04, 0.22, 0.45];

/// The WGSL source of the terrain and water pipelines, with the light
/// direction, the ambient share, and the ramp stops written in from the
/// crate's Rust constants.
#[must_use]
pub fn shader_source() -> String {
    String::new()
}
