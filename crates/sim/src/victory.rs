//! The match outcome (docs/adr/0014-vision-and-victory.md, SIM-084,
//! SIM-085).

use crate::ids::Army;

/// How a match ended: one army is the last standing, or none is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Winner(Army),
    Draw,
}
