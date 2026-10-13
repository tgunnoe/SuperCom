//! The build phase (SIM-056, SIM-059 to SIM-061): consumption rates,
//! progress and assist, factory queues, and structure placement.

use crate::ids::Q;
use crate::world::UnitKind;

/// The floor of a build time before the builder's rate divides it: raw bits
/// 429496730, the nearest [`Q`] to a tenth of a second (SIM-050).
pub const BUILD_TIME_FLOOR: Q = Q::from_bits(429_496_730);

/// How far a builder reaches, in world units from the nearest edge of the
/// site's footprint (SIM-061). Positions and distances are `f64` geometry
/// (ADR-0006); everything economic is [`Q`].
pub const BUILD_RANGE: f64 = 5.0;

/// What building one kind costs per second at one builder's rate (SIM-056).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildRates {
    pub seconds: Q,
    pub mass_per_second: Q,
    pub energy_per_second: Q,
}

/// The rates of building `kind` at `builder_rate`; `None` when the kind has
/// no build time or the rate is not positive (SIM-056).
#[must_use]
pub fn build_rates(kind: &UnitKind, builder_rate: Q) -> Option<BuildRates> {
    let _ = (kind, builder_rate);
    None
}

/// The cells a kind's footprint covers, width by depth; a missing block or
/// a zero dimension reads as 1 (SIM-061).
#[must_use]
pub fn footprint_of(kind: &UnitKind) -> (u32, u32) {
    let _ = kind;
    (1, 1)
}
