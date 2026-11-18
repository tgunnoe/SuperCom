//! Models and instances: plain data the renderer draws at transforms in
//! colours. A model is positions, normals, and triangle indices; an instance
//! is a model's index in the uploaded set, a transform, a colour, and a
//! style. The renderer learns nothing of what a model depicts.

use crate::Error;

/// Bytes of one model vertex as the renderer uploads it: position and
/// normal, six `f32`, the terrain's own layout.
pub const MODEL_VERTEX_BYTES: u64 = crate::VERTEX_BYTES;

/// Bytes of one 32-bit model index.
pub const MODEL_INDEX_BYTES: u64 = crate::INDEX_BYTES;

/// Bytes of one instance as the renderer uploads it: the transform's sixteen
/// `f32` and the colour's three.
pub const INSTANCE_BYTES: u64 = 76;

/// A triangle mesh in the model's own space: one unit normal per position
/// and 32-bit indices, three per triangle, counter-clockwise seen from
/// outside. Nothing is checked until [`check_model`] or an upload.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    /// One `[x, y, z]` per vertex.
    pub positions: Vec<[f32; 3]>,
    /// One unit normal per vertex, in the order of `positions`.
    pub normals: Vec<[f32; 3]>,
    /// Triangle corner indices into `positions`.
    pub indices: Vec<u32>,
}

/// How an instance is shaded and where it sits in the draw order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Style {
    /// Lit by the terrain's light, depth-tested, back faces culled.
    Lit,
    /// The colour as given, over everything, never hidden.
    Flat,
}

/// One drawn copy of a model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    /// The model's index in the uploaded set.
    pub model: usize,
    /// Model space to world space, column-major as in the crate's
    /// conventions: `transform[c][r]` is row `r` of column `c`.
    pub transform: [[f32; 4]; 4],
    /// Linear RGB, each channel in 0 to 1.
    pub color: [f32; 3],
    /// How the instance is shaded.
    pub style: Style,
}

/// Checks `model`, the `index`th of a set, without touching a device.
///
/// # Errors
/// [`Error::EmptyModel`] for no vertex or no index, [`Error::ModelNormalCount`]
/// for unequal position and normal counts, [`Error::ModelIndexCount`] for an
/// index count that is not a multiple of three, [`Error::ModelIndexRange`]
/// for an index at or past the vertex count, and [`Error::ModelNonFinite`]
/// for a position or a normal that is not finite, checked in that order.
pub fn check_model(_index: usize, _model: &Model) -> Result<(), Error> {
    Ok(())
}

/// Whether a model of `vertices` vertices and `indices` indices fits a
/// device whose largest buffer is `max_buffer_size` bytes: the vertex buffer
/// ([`MODEL_VERTEX_BYTES`] per vertex) and the index buffer
/// ([`MODEL_INDEX_BYTES`] per index) must each fit. Pure, so a caller runs
/// it before creating any buffer.
///
/// # Errors
/// [`Error::ModelTooLarge`] naming the first buffer that would exceed the
/// limit, the vertex buffer before the index buffer.
pub fn model_fits(_vertices: usize, _indices: usize, _max_buffer_size: u64) -> Result<(), Error> {
    Ok(())
}

/// Whether a list of `count` instances fits a device whose largest buffer
/// is `max_buffer_size` bytes, at [`INSTANCE_BYTES`] each. Pure.
///
/// # Errors
/// [`Error::InstancesTooLarge`] when the list's buffer would exceed the
/// limit.
pub fn instances_fit(_count: usize, _max_buffer_size: u64) -> Result<(), Error> {
    Ok(())
}

/// Checks a list of instances against a set of `models` models, without
/// touching a device.
///
/// # Errors
/// [`Error::InstanceModel`] for an instance naming no model of the set,
/// [`Error::InstanceTransform`] for a transform with a non-finite entry,
/// and [`Error::InstanceColor`] for a colour channel that is not finite or
/// is outside 0 to 1, the first offending instance in list order and, within
/// one instance, in that order.
pub fn check_instances(_instances: &[Instance], _models: usize) -> Result<(), Error> {
    Ok(())
}

/// The transform that places a model at `position`: it scales by `scale`,
/// then turns about the y axis, then translates. Headings and yaws are
/// measured as the simulation measures a heading, 0 along +x and increasing
/// toward +z; with `yaw` the model's own forward direction, the turn takes
/// every horizontal direction `(cos a, 0, sin a)` to
/// `(cos(a + heading - yaw), 0, sin(a + heading - yaw))` and leaves y alone,
/// so the model's forward direction ends along `(cos heading, 0, sin
/// heading)`. Because +z is south and y is up, this is the opposite of the
/// usual positive turn about +y. The matrix is column-major.
///
/// # Errors
/// [`Error::BadPlacement`] when `scale` is not finite and positive, or when
/// `heading`, `yaw`, or a coordinate of `position` is not finite; no matrix
/// it returns reverses a triangle's winding.
pub fn placement(
    _position: [f32; 3],
    _heading: f32,
    _scale: f32,
    _yaw: f32,
) -> Result<[[f32; 4]; 4], Error> {
    Ok([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// The linear RGB colour of a lit instance's surface whose unit normal is
/// `normal`: `color * (AMBIENT + (1 - AMBIENT) * max(n . L, 0))` with the
/// terrain's own [`LIGHT_DIRECTION`] and [`AMBIENT`].
#[must_use]
pub const fn shade_lit(color: [f32; 3], _normal: [f32; 3]) -> [f32; 3] {
    color
}
