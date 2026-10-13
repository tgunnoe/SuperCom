//! The economy phase (SIM-040, SIM-057, SIM-058): per-army settlement of
//! storage, income, requests, and the granted fraction.

use crate::ids::Q;

/// One army's settlement in the last economy phase; zeros, with both
/// fractions 1, before any phase has run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ledger {
    pub income_mass: Q,
    pub income_energy: Q,
    pub requested_mass: Q,
    pub requested_energy: Q,
    pub granted_mass: Q,
    pub granted_energy: Q,
    pub fraction_mass: Q,
    pub fraction_energy: Q,
}

impl Default for Ledger {
    fn default() -> Ledger {
        Ledger {
            income_mass: Q::ZERO,
            income_energy: Q::ZERO,
            requested_mass: Q::ZERO,
            requested_energy: Q::ZERO,
            granted_mass: Q::ZERO,
            granted_energy: Q::ZERO,
            fraction_mass: Q::ONE,
            fraction_energy: Q::ONE,
        }
    }
}

/// A per-second rate as the amount one tick applies: the value divided by
/// the ten ticks per second of SIM-001, in fixed point (SIM-057).
#[must_use]
pub fn per_tick(per_second: Q) -> Q {
    let _ = per_second;
    Q::ZERO
}
