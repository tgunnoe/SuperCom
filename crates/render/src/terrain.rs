//! The terrain grid.

use crate::Error;
use supercom_scmap::Map;

/// The terrain as a whole-heightmap triangle grid: one vertex per sample,
/// two triangles per cell. Coordinates are world units: x east along
/// columns, y up, z south along rows.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct TerrainMesh {
    /// Cells along x.
    pub columns: u32,
    /// Cells along z.
    pub rows: u32,
    /// One `[x, y, z]` per sample, row by row.
    pub positions: Vec<[f32; 3]>,
    /// One unit normal per vertex, in the order of `positions`.
    pub normals: Vec<[f32; 3]>,
    /// Triangle corner indices into `positions`, three per triangle,
    /// counter-clockwise seen from above.
    pub indices: Vec<u32>,
    /// The lowest vertex elevation, the lower end of the height range the
    /// normalization uses.
    pub lowest: f32,
    /// The highest vertex elevation.
    pub highest: f32,
    /// Extent along x in world units, from the map's header.
    pub width: f32,
    /// Extent along z in world units, from the map's header.
    pub height: f32,
}

/// The `f32` for a count: exact up to 2^24, and every installed cell count
/// (at most 4096) converts exactly; a count past 2^24 rounds.
#[expect(
    clippy::cast_precision_loss,
    reason = "std has no lossless u32 to f32 conversion; cell counts reach 4096"
)]
fn to_f32(n: u32) -> f32 {
    n as f32
}

/// The vertex count of a grid of `columns` by `rows` cells,
/// `(columns + 1) * (rows + 1)`.
///
/// # Errors
/// [`Error::TooManyVertices`] when the count exceeds `u32::MAX`, which
/// 32-bit indices cannot address.
pub fn grid_vertex_count(columns: u32, rows: u32) -> Result<u32, Error> {
    let too_many = Error::TooManyVertices { columns, rows };
    (u64::from(columns) + 1)
        .checked_mul(u64::from(rows) + 1)
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(too_many)
}

/// Bytes of one vertex as the renderer uploads it: position and normal, six
/// `f32`.
pub const VERTEX_BYTES: u64 = 24;

/// Bytes of one 32-bit index.
pub const INDEX_BYTES: u64 = 4;

/// Whether a grid of `columns` by `rows` cells fits a device whose largest
/// buffer is `max_buffer_size` bytes: both the vertex buffer
/// ([`VERTEX_BYTES`] per vertex) and the index buffer ([`INDEX_BYTES`] times
/// six per cell) must fit. Pure, so a caller runs it before building the
/// grid.
///
/// # Errors
/// [`Error::GridTooLarge`] naming the first buffer that would exceed the
/// limit, the vertex buffer before the index buffer.
pub fn grid_fits(_columns: u32, _rows: u32, _max_buffer_size: u64) -> Result<(), Error> {
    Ok(())
}

impl TerrainMesh {
    /// Builds the grid for `map`: vertex `(i, j)` at x = `i` times (map
    /// width / columns), z = `j` times (map height / rows), and y = sample
    /// times the stored scale.
    ///
    /// # Errors
    /// [`Error::ZeroCells`], [`Error::BadExtent`], [`Error::NonFiniteScale`],
    /// [`Error::TooManyVertices`], [`Error::SampleCount`],
    /// [`Error::NonFiniteElevation`], or [`Error::NonFiniteNormal`] when the
    /// map cannot form a grid.
    pub fn build(map: &Map) -> Result<Self, Error> {
        let heightmap = &map.heightmap;
        let (columns, rows) = (heightmap.width, heightmap.height);
        if columns == 0 || rows == 0 {
            return Err(Error::ZeroCells { columns, rows });
        }
        let (width, height) = (map.width, map.height);
        if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
            return Err(Error::BadExtent { width, height });
        }
        if !heightmap.scale.is_finite() {
            return Err(Error::NonFiniteScale {
                scale: heightmap.scale,
            });
        }
        let vertex_count = grid_vertex_count(columns, rows)?;
        let grid = Grid { columns, rows };
        if grid.slot(vertex_count)? != heightmap.samples.len() {
            return Err(Error::SampleCount {
                expected: u64::from(vertex_count),
                found: heightmap.samples.len(),
            });
        }

        let heights: Vec<f32> = heightmap
            .samples
            .iter()
            .map(|&sample| f32::from(sample) * heightmap.scale)
            .collect();
        if heights.iter().any(|h| !h.is_finite()) {
            return Err(Error::NonFiniteElevation);
        }
        let lowest = heights.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        let stride = columns + 1;
        let cell_x = width / to_f32(columns);
        let cell_z = height / to_f32(rows);

        let mut positions = Vec::with_capacity(heights.len());
        for j in 0..=rows {
            for i in 0..stride {
                let at = grid.slot(j * stride + i)?;
                positions.push([to_f32(i) * cell_x, heights[at], to_f32(j) * cell_z]);
            }
        }

        let normals = normals(&heights, grid, cell_x, cell_z)?;
        let indices = indices(grid)?;

        Ok(Self {
            columns,
            rows,
            positions,
            normals,
            indices,
            lowest,
            highest,
            width,
            height,
        })
    }
}

/// The cell counts of a grid whose vertex count already fits 32 bits.
#[derive(Clone, Copy)]
struct Grid {
    columns: u32,
    rows: u32,
}

impl Grid {
    /// A grid position as a slice index; fails only where `usize` is
    /// narrower than 32 bits, which makes the grid too large to address.
    fn slot(self, n: u32) -> Result<usize, Error> {
        usize::try_from(n).map_err(|_| Error::TooManyVertices {
            columns: self.columns,
            rows: self.rows,
        })
    }
}

/// Unit normals from the height differences across each vertex along x and
/// z (one-sided at an edge): `(-dh/dx, 1, -dh/dz)` normalized.
fn normals(heights: &[f32], grid: Grid, cell_x: f32, cell_z: f32) -> Result<Vec<[f32; 3]>, Error> {
    let Grid { columns, rows } = grid;
    let stride = columns + 1;
    let mut out = Vec::with_capacity(heights.len());
    for j in 0..=rows {
        let (up, down) = (j.saturating_sub(1), (j + 1).min(rows));
        for i in 0..stride {
            let (left, right) = (i.saturating_sub(1), (i + 1).min(columns));
            let h = |col: u32, row: u32| -> Result<f32, Error> {
                Ok(heights[grid.slot(row * stride + col)?])
            };
            let dx = (h(right, j)? - h(left, j)?) / (to_f32(right - left) * cell_x);
            let dz = (h(i, down)? - h(i, up)?) / (to_f32(down - up) * cell_z);
            let length = (dx * dx + dz * dz + 1.0).sqrt();
            let normal = [-dx / length, 1.0 / length, -dz / length];
            if !normal.iter().all(|c| c.is_finite()) {
                return Err(Error::NonFiniteNormal);
            }
            out.push(normal);
        }
    }
    Ok(out)
}

/// Two triangles per cell, split from the north-east vertex to the
/// south-west one and wound counter-clockwise seen from above.
fn indices(grid: Grid) -> Result<Vec<u32>, Error> {
    let Grid { columns, rows } = grid;
    let stride = columns + 1;
    let count = u64::from(columns) * u64::from(rows) * 6;
    let capacity = usize::try_from(count).map_err(|_| Error::TooManyVertices { columns, rows })?;
    let mut out = Vec::with_capacity(capacity);
    for j in 0..rows {
        for i in 0..columns {
            let north_west = j * stride + i;
            let north_east = north_west + 1;
            let south_west = north_west + stride;
            let south_east = south_west + 1;
            out.extend_from_slice(&[
                north_west, south_west, north_east, north_east, south_west, south_east,
            ]);
        }
    }
    Ok(out)
}
