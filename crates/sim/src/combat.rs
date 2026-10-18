//! The combat seam: weapon state, projectiles, the per-tick hand-offs
//! between the combat phases, and the pure helpers the phases share
//! (docs/adr/0013-combat.md, SIM-075 to SIM-081).

use std::collections::BTreeSet;

use crate::ids::{Army, EntityId, Pos, Q};
use crate::world::World;

/// How near a projectile counts as touching its target, in world units
/// (SIM-077).
pub const IMPACT_RADIUS: f64 = 0.5;

/// The default gravitational acceleration along y, in world units per
/// second squared: the pinned engine stub's documented default (SIM-073).
pub const GRAVITY: f64 = -4.9;

/// One weapon's state on an entity, in the order of its kind's weapons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponState {
    /// The entity the weapon is aimed at.
    pub target: Option<EntityId>,
    /// Ticks until the weapon may fire again; 0 when ready (SIM-076).
    pub cooldown: u32,
}

/// A projectile identifier; ids ascend from 1 in launch order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectileId(pub u32);

/// A projectile in flight (SIM-077).
#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub id: ProjectileId,
    pub army: Army,
    pub source: EntityId,
    /// The projectile kind's id; `None` for an instant hit.
    pub kind: Option<String>,
    pub pos: Pos,
    pub vel: [f64; 3],
    pub target: Option<EntityId>,
    /// The target's position when fired, kept for a target that dies.
    pub aim: Pos,
    pub damage: Q,
    pub damage_radius: f64,
    pub damage_type: String,
    /// Seconds left; `None` for an instant hit.
    pub lifetime: Option<f64>,
}

/// One impact the projectiles phase hands to the damage phase.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Impact {
    pub pos: Pos,
    /// The entity the impact was on, when it was on one.
    pub on: Option<EntityId>,
    pub damage: Q,
    pub damage_radius: f64,
    pub damage_type: String,
    pub army: Army,
}

/// Values that live for one tick only: the step creates them and passes
/// them to the phases, so nothing survives a step by construction.
#[derive(Debug, Default)]
pub(crate) struct Scratch {
    /// The impacts of the projectiles phase, in projectile-id order.
    pub impacts: Vec<Impact>,
    /// The entities under construction the damage phase marked for death.
    pub marked: BTreeSet<EntityId>,
}

/// The weapon states of a new entity of `blueprint`: one per weapon of its
/// kind, none for an unknown blueprint.
pub(crate) fn fresh_weapons(world: &World, blueprint: &str) -> Vec<WeaponState> {
    let count = world.units.get(blueprint).map_or(0, |k| k.weapons.len());
    vec![WeaponState::default(); count]
}

/// The ticks between shots of a weapon of `rate_of_fire` shots per second:
/// the ceiling of ten over the rate, at least 1 (SIM-076). A rate of zero
/// gives the longest cooldown; a negative, negative-zero, or undefined rate
/// gives 1.
#[must_use]
pub fn cooldown_ticks(rate_of_fire: f64) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the cast saturates: a ceiling above u32::MAX is the longest cooldown, and a negative or undefined one is raised to 1 below"
    )]
    let ticks = (10.0 / rate_of_fire).ceil() as u32;
    ticks.max(1)
}

/// The planar distance between two positions, ignoring y.
#[must_use]
pub fn distance_xz(a: &Pos, b: &Pos) -> f64 {
    let (dx, dz) = (a.x - b.x, a.z - b.z);
    libm::sqrt(dx * dx + dz * dz)
}
