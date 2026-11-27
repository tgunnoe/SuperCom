//! Headless core of the first-light renderer: the terrain grid built from a
//! parsed map, the water plane, the camera, the shading function, and the
//! models and instances drawn over them, all
//! pure functions over plain `f32` arrays, and the GPU side that draws them
//! with wgpu into any texture view. Nothing here opens a window or links a
//! windowing or math crate; the windowed mode calls the one draw function.
//!
//! Conventions, shared by every item: x runs along heightmap columns (east),
//! y is up, z runs along heightmap rows (south), right-handed, in world
//! units. Matrices are column-major, `m[c][r]` being row `r` of column `c`.
//! Clip space has y up and depth 0 at the near plane and 1 at the far plane.

mod camera;
mod error;
mod gate;
mod model;
mod model_shader;
mod renderer;
mod shader;
mod shading;
mod terrain;
mod texture;
mod textured_shader;
mod water;

pub use camera::{
    Camera, FAST_ZOOM_STEP, FOV_Y_RADIANS, NEAR_DISTANCE, NEAR_PITCH_RADIANS, NORMAL_ZOOM_STEP,
    ZoomSpeed,
};
pub use error::{Error, TextureFault, TextureItem};
pub use gate::{GpuGate, gpu_gate};
pub use model::{
    INSTANCE_BYTES, Instance, Model, Style, check_instances, check_model, instances_fit,
    model_fits, placement, shade_lit,
};
pub use model_shader::model_shader_source;
pub use renderer::{Renderer, SurfaceRenderer, surface_format};
pub use shader::{CLEAR_COLOR, WATER_COLOR, shader_source};
pub use shading::{AMBIENT, LIGHT_DIRECTION, RAMP, normalize_height, shade};
pub use terrain::{INDEX_BYTES, TerrainMesh, VERTEX_BYTES, grid_fits, grid_vertex_count};
pub use texture::{
    Blend, Filtering, LAYER_SLOTS, Layer, SAMPLED_TEXTURES, TerrainTextures, Texel, TextureLevel,
    blend_stored, check_textures, layer_coordinate, mask_coordinate, mask_weight, srgb_decode,
    textured_color,
};
pub use textured_shader::textured_shader_source;
pub use water::WaterPlane;
/// The graphics library the renderer is built on, re-exported so an
/// application names its types without a dependency line of its own.
pub use wgpu;
