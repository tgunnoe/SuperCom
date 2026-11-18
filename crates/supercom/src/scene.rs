//! What is drawn: the instance list, a pure function of the state and the
//! values kept beside it.

use crate::interp::{Kept, blend_heading, blend_position};
use crate::markers::{COMMANDER_MODEL, MOBILE_MODEL, PROJECTILE_MODEL, STATIC_MODEL};
use std::collections::BTreeMap;
use supercom_render::{Error, Instance, Style, TerrainMesh, placement};
use supercom_sim::{Army, Layer, MotionType, Pos, State, UnitKind, footprint_of};

/// The share of the camera's distance a marker never shrinks below.
pub const SIZE_SHARE: f32 = 0.02;

/// A projectile's marker, in world units across.
pub const PROJECTILE_SIZE: f32 = 0.5;

/// A projectile's linear colour.
pub const PROJECTILE_COLOUR: [f32; 3] = [1.0, 0.95, 0.5];

/// The least an unfinished entity's colour is multiplied by.
pub const DIM_FLOOR: f64 = 0.4;

/// The linear colours of the armies, in the order of the armies' numbers.
const ARMY_COLOURS: [[f32; 3]; 8] = [
    [0.9, 0.1, 0.1],
    [0.1, 0.3, 0.9],
    [0.1, 0.7, 0.2],
    [0.9, 0.8, 0.1],
    [0.6, 0.1, 0.8],
    [0.1, 0.8, 0.8],
    [0.9, 0.5, 0.1],
    [0.9, 0.9, 0.9],
];

/// The category that marks a commander.
const COMMAND_CATEGORY: &str = "COMMAND";

/// An army's linear colour: entry `(n - 1) mod 8` of a table of eight, and
/// the first for army 0.
#[must_use]
pub fn army_colour(army: Army) -> [f32; 3] {
    let index = usize::from(army.0.saturating_sub(1)) % ARMY_COLOURS.len();
    ARMY_COLOURS.get(index).copied().unwrap_or([1.0; 3])
}

/// `colour` dimmed for a build `progress` from 0 to 1 by
/// `DIM_FLOOR + (1 - DIM_FLOOR) * progress`.
#[must_use]
pub fn dim(colour: [f32; 3], progress: f64) -> [f32; 3] {
    let factor = to_f32(DIM_FLOOR + (1.0 - DIM_FLOOR) * progress.clamp(0.0, 1.0));
    colour.map(|channel| channel * factor)
}

/// A kind's own size in world units: the larger side of its footprint, 1
/// without one.
#[must_use]
pub fn own_size(kind: &UnitKind) -> f32 {
    let (width, depth) = footprint_of(kind);
    to_f32(f64::from(width.max(depth)))
}

/// A marker's size: the larger of the unit's own size and
/// [`SIZE_SHARE`] times the camera's distance.
#[must_use]
pub fn marker_size(own: f32, camera_distance: f32) -> f32 {
    own.max(SIZE_SHARE * camera_distance)
}

/// The marker model and the own size of an entity's kind; a kind the world
/// does not know is a complete kind that does not move and is 1 across.
#[must_use]
pub fn shape_of(kind: Option<&UnitKind>) -> (usize, f32) {
    let Some(kind) = kind else {
        return (STATIC_MODEL, 1.0);
    };
    let model = if kind.categories.contains(COMMAND_CATEGORY) {
        COMMANDER_MODEL
    } else if kind.motion == MotionType::None {
        STATIC_MODEL
    } else {
        MOBILE_MODEL
    };
    (model, own_size(kind))
}

/// A simulation value as `f32`; positions and sizes are far inside its
/// range, and the renderer's arithmetic is `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "std has no f64 to f32 conversion; the values are map coordinates"
)]
fn to_f32(value: f64) -> f32 {
    value as f32
}

/// The `f32` position of an interpolated simulation position.
fn position(pos: Pos) -> [f32; 3] {
    [to_f32(pos.x), to_f32(pos.y), to_f32(pos.z)]
}

/// The instance list of `state` at `fraction` of a tick past `kept`: every
/// entity in id order, then every projectile in id order, each flat.
///
/// # Errors
/// The renderer's [`Error`] when a placement is refused, which a finite
/// state never causes.
pub fn instances(
    state: &State,
    kept: &Kept,
    fraction: f64,
    kinds: &BTreeMap<String, UnitKind>,
    mesh: &TerrainMesh,
    camera_distance: f32,
) -> Result<Vec<Instance>, Error> {
    let mut list = Vec::with_capacity(state.entities.len() + state.projectiles.len());
    for entity in state.entities.values() {
        let (pos, heading) = match kept.entity(entity.id) {
            Some((from, from_heading)) => (
                blend_position(from, entity.pos, fraction),
                blend_heading(from_heading, entity.heading, fraction),
            ),
            None => (entity.pos, entity.heading),
        };
        let mut at = position(pos);
        if matches!(entity.layer, Layer::Land | Layer::Seabed)
            && let Some(height) = mesh.height_at(at[0], at[2])
        {
            at[1] = height;
        }
        let kind = kinds.get(&entity.blueprint);
        let (model, own) = shape_of(kind);
        let colour = army_colour(entity.army);
        let colour = match kind {
            Some(kind) if !entity.complete => dim(colour, entity.progress(kind).to_num::<f64>()),
            _ => colour,
        };
        list.push(Instance {
            model,
            transform: placement(at, to_f32(heading), marker_size(own, camera_distance), 0.0)?,
            color: colour,
            style: Style::Flat,
        });
    }
    for projectile in state.projectiles.values() {
        let pos = match kept.projectile(projectile.id) {
            Some(from) => blend_position(from, projectile.pos, fraction),
            None => projectile.pos,
        };
        list.push(Instance {
            model: PROJECTILE_MODEL,
            transform: placement(position(pos), 0.0, PROJECTILE_SIZE, 0.0)?,
            color: PROJECTILE_COLOUR,
            style: Style::Flat,
        });
    }
    Ok(list)
}

#[cfg(test)]
mod tests;
