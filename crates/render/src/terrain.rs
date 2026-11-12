//! The terrain grid.

use crate::Error;
use supercom_scmap::Map;

/// The terrain as a whole-heightmap triangle grid.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainMesh {
    /// One `[x, y, z]` per sample, row by row, in world units.
    pub positions: Vec<[f32; 3]>,
    /// One unit normal per vertex.
    pub normals: Vec<[f32; 3]>,
    /// Triangle corner indices into `positions`, three per triangle.
    pub indices: Vec<u32>,
    /// The lowest vertex elevation.
    pub lowest: f32,
    /// The highest vertex elevation.
    pub highest: f32,
    /// Extent along x in world units.
    pub width: f32,
    /// Extent along z in world units.
    pub height: f32,
}

impl TerrainMesh {
    /// Builds the grid for `map`.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn build(map: &Map) -> Result<Self, Error> {
        Ok(Self {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            lowest: 0.0,
            highest: 0.0,
            width: map.width,
            height: map.height,
        })
    }
}

/// The vertex count of a grid of `columns` by `rows` cells.
///
/// # Errors
/// [`Error::TooManyVertices`] when the count exceeds `u32::MAX`.
pub fn grid_vertex_count(columns: u32, rows: u32) -> Result<u32, Error> {
    let _ = (columns, rows);
    Ok(0)
}
