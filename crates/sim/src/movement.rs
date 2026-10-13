//! The movement phase (SIM-064 to SIM-070): layer assignment, route
//! following, and the geometry helpers the tests share.

use crate::build::BUILD_RANGE;
use crate::ids::Pos;

/// One tick in seconds: a tenth, from SIM-001 and SIM-002 (SIM-069).
pub const TICK_SECONDS: f64 = 0.1;

/// How near a cell centre or a goal counts as reached, in world units
/// (SIM-068).
pub const ARRIVAL_RADIUS: f64 = 0.5;

/// How near a guard stays to its target, equal to the build range
/// (SIM-068).
pub const FOLLOW_RANGE: f64 = BUILD_RANGE;

/// The cell holding a position: the floor of x and z; `None` when either is
/// negative, not a number, or beyond `u32` (SIM-064).
#[must_use]
pub fn cell_of(_pos: &Pos) -> Option<(u32, u32)> {
    None
}

/// The centre of a cell in world units: each coordinate plus a half
/// (SIM-064).
#[must_use]
pub fn cell_centre(_cell: (u32, u32)) -> (f64, f64) {
    (0.0, 0.0)
}
