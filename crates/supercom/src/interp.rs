//! Interpolation over one tick: the values kept from before the last step
//! and the blends toward the current ones.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "wired by the code that follows the tests")
)]

use std::collections::BTreeMap;
use supercom_sim::{EntityId, Pos, ProjectileId, State};

/// The positions and headings of the entities and the positions of the
/// projectiles as they were at one moment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kept {
    entities: BTreeMap<EntityId, (Pos, f64)>,
    projectiles: BTreeMap<ProjectileId, Pos>,
}

impl Kept {
    /// The values `state` holds now.
    #[must_use]
    pub fn of(state: &State) -> Self {
        let _ = state;
        Self::default()
    }

    /// The kept position and heading of an entity, if it was kept.
    #[must_use]
    pub fn entity(&self, id: EntityId) -> Option<(Pos, f64)> {
        self.entities.get(&id).copied()
    }

    /// The kept position of a projectile, if it was kept.
    #[must_use]
    pub fn projectile(&self, id: ProjectileId) -> Option<Pos> {
        self.projectiles.get(&id).copied()
    }
}

/// `from` plus the difference to `to` times `fraction`, componentwise.
#[must_use]
pub fn blend_position(from: Pos, to: Pos, fraction: f64) -> Pos {
    let _ = (from, fraction);
    to
}

/// `from` plus the difference to `to`, taken the short way round, times
/// `fraction`; headings are radians.
#[must_use]
pub fn blend_heading(from: f64, to: f64, fraction: f64) -> f64 {
    let _ = (to, fraction);
    from
}

#[cfg(test)]
mod tests;
