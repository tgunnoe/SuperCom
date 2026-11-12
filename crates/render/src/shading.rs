//! Height normalization and the terrain shading function.

/// Unit vector from a surface toward the light, in world axes (x east, y up,
/// z south): from the north-west and above.
pub const LIGHT_DIRECTION: [f32; 3] = [-0.36, 0.8, -0.48];

/// Share of the light that reaches a surface turned away from it.
const AMBIENT: f32 = 0.3;

/// Linear RGB ramp stops at normalized heights 0, 0.5, and 1.
const RAMP: [[f32; 3]; 3] = [[0.18, 0.30, 0.12], [0.50, 0.42, 0.28], [0.92, 0.90, 0.86]];

/// Maps `elevation` to `(elevation - lowest) / (highest - lowest)`, in
/// world units; a range of zero width, or a result that is not finite, maps
/// to 0. The result is not clamped: an elevation outside the range maps
/// outside 0 to 1.
#[must_use]
pub fn normalize_height(elevation: f32, lowest: f32, highest: f32) -> f32 {
    let range = highest - lowest;
    if range > 0.0 {
        let ratio = (elevation - lowest) / range;
        if ratio.is_finite() {
            return ratio;
        }
    }
    0.0
}

/// Linear RGB colour, each channel in 0 to 1, of a surface at normalized
/// height `height` (0 to 1, as [`normalize_height`] returns; values outside
/// are clamped) whose unit normal is `normal`: the height ramp scaled by an
/// ambient share plus the share of [`LIGHT_DIRECTION`] the normal faces.
#[must_use]
pub fn shade(height: f32, normal: [f32; 3]) -> [f32; 3] {
    let t = height.clamp(0.0, 1.0) * 2.0;
    let (low, high, local) = if t < 1.0 {
        (RAMP[0], RAMP[1], t)
    } else {
        (RAMP[1], RAMP[2], t - 1.0)
    };
    let facing = (normal[0] * LIGHT_DIRECTION[0]
        + normal[1] * LIGHT_DIRECTION[1]
        + normal[2] * LIGHT_DIRECTION[2])
        .max(0.0);
    let light = AMBIENT + (1.0 - AMBIENT) * facing;
    let channel = |k: usize| ((low[k] + (high[k] - low[k]) * local) * light).clamp(0.0, 1.0);
    [channel(0), channel(1), channel(2)]
}
