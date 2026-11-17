//! The water plane.

use supercom_scmap::Map;

/// One horizontal quad at the water surface, spanning the map's extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterPlane {
    /// The four corners `[x, y, z]` in world units, counter-clockwise seen
    /// from above, so the quad faces up; `y` is the surface elevation.
    pub corners: [[f32; 3]; 4],
}

impl WaterPlane {
    /// The plane for `map`: its stored surface elevation over the header's
    /// extent. `None` when the map carries no water, or when the elevation
    /// or the extent is not finite and positive, which no plane can draw.
    #[must_use]
    pub fn from_map(map: &Map) -> Option<Self> {
        let elevation = map.water?.elevation;
        let (width, height) = (map.width, map.height);
        let extent_ok = width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0;
        (elevation.is_finite() && extent_ok).then_some(Self {
            corners: [
                [0.0, elevation, 0.0],
                [0.0, elevation, height],
                [width, elevation, height],
                [width, elevation, 0.0],
            ],
        })
    }
}
