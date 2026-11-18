//! Models and instances: plain data the renderer draws at transforms in
//! colours. A model is positions, normals, and triangle indices; an instance
//! is a model's index in the uploaded set, a transform, a colour, and a
//! style. The renderer learns nothing of what a model depicts.

use crate::{AMBIENT, Error, INDEX_BYTES, LIGHT_DIRECTION, VERTEX_BYTES};

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

fn finite3(v: [f32; 3]) -> bool {
    v.iter().all(|c| c.is_finite())
}

/// Whether a normal's squared length, in `f64`, is below [`MIN_NORMAL_SQUARED`].
fn short(normal: [f32; 3]) -> bool {
    normal
        .iter()
        .map(|c| f64::from(*c) * f64::from(*c))
        .sum::<f64>()
        < MIN_NORMAL_SQUARED
}

/// The least squared length of a normal [`check_model`] accepts.
const MIN_NORMAL_SQUARED: f64 = 1e-12;

/// Checks `model`, the `index`th of a set, without touching a device.
///
/// # Errors
/// [`Error::EmptyModel`] for no vertex or no index, [`Error::ModelNormalCount`]
/// for unequal position and normal counts, [`Error::ModelIndexCount`] for an
/// index count that is not a multiple of three, [`Error::ModelIndexRange`]
/// for an index at or past the vertex count, and [`Error::ModelNonFinite`]
/// for a position or a normal that is not finite, and
/// [`Error::ModelShortNormal`] for a normal whose squared length is below
/// 1e-12, checked in that order.
pub fn check_model(index: usize, model: &Model) -> Result<(), Error> {
    let vertices = model.positions.len();
    if vertices == 0 || model.indices.is_empty() {
        return Err(Error::EmptyModel { model: index });
    }
    if model.normals.len() != vertices {
        return Err(Error::ModelNormalCount {
            model: index,
            positions: vertices,
            normals: model.normals.len(),
        });
    }
    if model.indices.len() % 3 != 0 {
        return Err(Error::ModelIndexCount {
            model: index,
            indices: model.indices.len(),
        });
    }
    if let Some(&bad) = model
        .indices
        .iter()
        .find(|&&i| !usize::try_from(i).is_ok_and(|i| i < vertices))
    {
        return Err(Error::ModelIndexRange {
            model: index,
            index: bad,
            vertices,
        });
    }
    let vertex = model
        .positions
        .iter()
        .zip(&model.normals)
        .position(|(p, n)| !(finite3(*p) && finite3(*n)));
    if let Some(vertex) = vertex {
        return Err(Error::ModelNonFinite {
            model: index,
            vertex,
        });
    }
    match model.normals.iter().position(|n| short(*n)) {
        Some(vertex) => Err(Error::ModelShortNormal {
            model: index,
            vertex,
        }),
        None => Ok(()),
    }
}

fn saturating(bytes: u128) -> u64 {
    u64::try_from(bytes).unwrap_or(u64::MAX)
}

fn widen(n: usize) -> u128 {
    u128::try_from(n).unwrap_or(u128::MAX)
}

/// Whether a model of `vertices` vertices and `indices` indices fits a
/// device whose largest buffer is `max_buffer_size` bytes: the vertex buffer
/// ([`VERTEX_BYTES`] per vertex) and the index buffer
/// ([`INDEX_BYTES`] per index) must each fit. Pure, so a caller runs
/// it before creating any buffer.
///
/// # Errors
/// [`Error::ModelTooLarge`] naming the first buffer that would exceed the
/// limit, the vertex buffer before the index buffer.
pub fn model_fits(vertices: usize, indices: usize, max_buffer_size: u64) -> Result<(), Error> {
    for (buffer, bytes) in [
        (
            "vertex",
            widen(vertices).saturating_mul(u128::from(VERTEX_BYTES)),
        ),
        (
            "index",
            widen(indices).saturating_mul(u128::from(INDEX_BYTES)),
        ),
    ] {
        if bytes > u128::from(max_buffer_size) {
            return Err(Error::ModelTooLarge {
                vertices,
                indices,
                buffer,
                bytes: saturating(bytes),
                limit: max_buffer_size,
            });
        }
    }
    Ok(())
}

/// Whether a list of `count` instances fits a device whose largest buffer
/// is `max_buffer_size` bytes, at [`INSTANCE_BYTES`] each. Pure.
///
/// # Errors
/// [`Error::InstancesTooLarge`] when the list's buffer would exceed the
/// limit.
pub fn instances_fit(count: usize, max_buffer_size: u64) -> Result<(), Error> {
    let bytes = widen(count).saturating_mul(u128::from(INSTANCE_BYTES));
    if bytes > u128::from(max_buffer_size) {
        return Err(Error::InstancesTooLarge {
            count,
            bytes: saturating(bytes),
            limit: max_buffer_size,
        });
    }
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
pub fn check_instances(instances: &[Instance], models: usize) -> Result<(), Error> {
    for (at, instance) in instances.iter().enumerate() {
        if instance.model >= models {
            return Err(Error::InstanceModel {
                instance: at,
                model: instance.model,
                models,
            });
        }
        if !instance
            .transform
            .iter()
            .all(|column| column.iter().all(|c| c.is_finite()))
        {
            return Err(Error::InstanceTransform { instance: at });
        }
        if !instance.color.iter().all(|c| (0.0..=1.0).contains(c)) {
            return Err(Error::InstanceColor { instance: at });
        }
    }
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
    position: [f32; 3],
    heading: f32,
    scale: f32,
    yaw: f32,
) -> Result<[[f32; 4]; 4], Error> {
    let bad = |what: &'static str| Err(Error::BadPlacement { what });
    if !(scale.is_finite() && scale > 0.0) {
        return bad("scale");
    }
    if !heading.is_finite() {
        return bad("heading");
    }
    if !yaw.is_finite() {
        return bad("yaw");
    }
    if !finite3(position) {
        return bad("position");
    }
    let (sin, cos) = (heading - yaw).sin_cos();
    Ok([
        [scale * cos, 0.0, scale * sin, 0.0],
        [0.0, scale, 0.0, 0.0],
        [-scale * sin, 0.0, scale * cos, 0.0],
        [position[0], position[1], position[2], 1.0],
    ])
}

/// The linear RGB colour of a lit instance's surface whose unit normal is
/// `normal`: `color * (AMBIENT + (1 - AMBIENT) * max(n . L, 0))` with the
/// terrain's own [`LIGHT_DIRECTION`] and [`AMBIENT`].
#[must_use]
pub fn shade_lit(color: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let facing = (normal[0] * LIGHT_DIRECTION[0]
        + normal[1] * LIGHT_DIRECTION[1]
        + normal[2] * LIGHT_DIRECTION[2])
        .max(0.0);
    let light = AMBIENT + (1.0 - AMBIENT) * facing;
    color.map(|c| c * light)
}
