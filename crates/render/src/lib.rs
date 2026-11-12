//! Headless core of the first-light renderer: the terrain grid built from a
//! parsed map, the water plane, the camera, and the shading function, all
//! pure functions over plain `f32` arrays. Nothing here draws, opens a
//! window, or links a graphics or math crate; the GPU side imports what this
//! crate makes public.
//!
//! Conventions, shared by every item: x runs along heightmap columns (east),
//! y is up, z runs along heightmap rows (south), right-handed, in world
//! units. Matrices are column-major, `m[c][r]` being row `r` of column `c`.
//! Clip space has y up and depth 0 at the near plane and 1 at the far plane.

mod camera;
mod error;
mod shading;
mod terrain;
mod water;

pub use camera::{
    Camera, FAST_ZOOM_STEP, FOV_Y_RADIANS, NEAR_DISTANCE, NEAR_PITCH_RADIANS, NORMAL_ZOOM_STEP,
    ZoomSpeed,
};
pub use error::Error;
pub use shading::{LIGHT_DIRECTION, normalize_height, shade};
pub use terrain::{TerrainMesh, grid_vertex_count};
pub use water::WaterPlane;
