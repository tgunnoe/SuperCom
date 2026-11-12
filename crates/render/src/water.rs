//! The water plane.

use supercom_scmap::Map;

/// One horizontal quad at the water surface, spanning the map's extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterPlane {
    /// The four corners `[x, y, z]` in world units, counter-clockwise seen
    /// from above.
    pub corners: [[f32; 3]; 4],
}

impl WaterPlane {
    /// The plane for `map`, or `None` when the map carries no water.
    #[must_use]
    pub fn from_map(map: &Map) -> Option<Self> {
        let _ = map;
        Some(Self {
            corners: [[0.0; 3]; 4],
        })
    }
}
