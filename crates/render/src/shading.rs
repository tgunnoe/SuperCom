//! Height normalization and the terrain shading function.

/// Unit vector from a surface toward the light, in world axes.
pub const LIGHT_DIRECTION: [f32; 3] = [0.0, 0.0, 0.0];

/// Maps `elevation` to `(elevation - lowest) / (highest - lowest)`; a range
/// of zero width maps to 0.
#[must_use]
pub fn normalize_height(elevation: f32, lowest: f32, highest: f32) -> f32 {
    let _ = (elevation, lowest, highest);
    f32::NAN
}

/// Linear RGB colour, each channel in 0 to 1, of a surface at normalized
/// height `height` whose unit normal is `normal`.
#[must_use]
pub fn shade(height: f32, normal: [f32; 3]) -> [f32; 3] {
    let _ = (height, normal);
    [f32::NAN; 3]
}
