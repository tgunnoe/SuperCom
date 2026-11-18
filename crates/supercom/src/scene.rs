//! What is drawn: the instance list, a pure function of the state and the
//! values kept beside it.

#![expect(dead_code, reason = "wired by the code that follows the tests")]

use crate::interp::Kept;
use crate::markers::STATIC_MODEL;
use std::collections::BTreeMap;
use supercom_render::{Error, Instance, TerrainMesh};
use supercom_sim::{Army, State, UnitKind};

/// The share of the camera's distance a marker never shrinks below.
pub const SIZE_SHARE: f32 = 0.02;

/// A projectile's marker, in world units across.
pub const PROJECTILE_SIZE: f32 = 0.5;

/// A projectile's linear colour.
pub const PROJECTILE_COLOUR: [f32; 3] = [1.0, 0.95, 0.5];

/// The least an unfinished entity's colour is multiplied by.
pub const DIM_FLOOR: f64 = 0.4;

/// An army's linear colour: entry `(n - 1) mod 8` of a table of eight, and
/// the first for army 0.
#[must_use]
pub fn army_colour(army: Army) -> [f32; 3] {
    let _ = army;
    [0.0; 3]
}

/// `colour` dimmed for a build `progress` from 0 to 1 by
/// `DIM_FLOOR + (1 - DIM_FLOOR) * progress`.
#[must_use]
pub fn dim(colour: [f32; 3], progress: f64) -> [f32; 3] {
    let _ = progress;
    colour
}

/// A kind's own size in world units: the larger side of its footprint, 1
/// without one.
#[must_use]
pub fn own_size(kind: &UnitKind) -> f32 {
    let _ = kind;
    0.0
}

/// A marker's size: the larger of the unit's own size and
/// [`SIZE_SHARE`] times the camera's distance.
#[must_use]
pub fn marker_size(own: f32, camera_distance: f32) -> f32 {
    let _ = camera_distance;
    own
}

/// The marker model and the own size of an entity's kind; a kind the world
/// does not know is a complete kind that does not move and is 1 across.
#[must_use]
pub fn shape_of(kind: Option<&UnitKind>) -> (usize, f32) {
    let _ = kind;
    (STATIC_MODEL, 1.0)
}

/// The instance list of `state` at `fraction` of a tick past `kept`.
///
/// # Errors
/// The renderer's [`Error`] when a placement is refused, which a finite
/// state never causes.
#[expect(
    clippy::unnecessary_wraps,
    reason = "a seam that returns the wrong answer; the behaviour that follows can fail"
)]
pub fn instances(
    state: &State,
    kept: &Kept,
    fraction: f64,
    kinds: &BTreeMap<String, UnitKind>,
    mesh: &TerrainMesh,
    camera_distance: f32,
) -> Result<Vec<Instance>, Error> {
    let _ = (state, kept, fraction, kinds, mesh, camera_distance);
    Ok(Vec::new())
}

#[cfg(test)]
mod tests;
