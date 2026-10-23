//! The economy phase (SIM-040, SIM-057, SIM-058): per-army settlement of
//! storage, income, requests, and the granted fraction.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::build::{build_rates, kind_of};
use crate::ids::{Army, EntityId, Q};
use crate::state::State;
use crate::world::World;

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
    // Dividing by a nonzero integer cannot fail; the fallback is unreachable.
    per_second.checked_div_int(10).unwrap_or(Q::ZERO)
}

/// The fraction of `requested` the `available` amount covers: 1 when nothing
/// is requested, else the lesser of 1 and available over requested.
fn fraction_of(available: Q, requested: Q) -> Q {
    if requested <= Q::ZERO {
        Q::ONE
    } else {
        Q::ONE.min(available.max(Q::ZERO).saturating_div(requested))
    }
}

/// What one entity asks of its army this tick.
#[derive(Clone, Copy, Default)]
struct Request {
    mass: Q,
    energy: Q,
}

impl Request {
    /// The lowest fraction among the resources requested (SIM-040).
    fn fraction(self, mass: Q, energy: Q) -> Q {
        let mut f = Q::ONE;
        if self.mass > Q::ZERO {
            f = f.min(mass);
        }
        if self.energy > Q::ZERO {
            f = f.min(energy);
        }
        f
    }
}

impl State {
    /// Settle every army in slot order (SIM-040, SIM-058).
    pub(crate) fn run_economy(&mut self) {
        let world = Arc::clone(self.world());
        let slots: Vec<Army> = self.armies.keys().copied().collect();
        let mut stalled = false;
        for slot in slots {
            stalled |= self.settle(&world, slot);
        }
        if stalled {
            self.counters.stall_ticks = self.counters.stall_ticks.saturating_add(1);
        }
    }

    /// Settle one army; true when either resource's fraction is below 1.
    fn settle(&mut self, world: &World, slot: Army) -> bool {
        let mut storage = Request::default();
        let mut income = Request::default();
        let mut requests: BTreeMap<EntityId, Request> = BTreeMap::new();
        // What each entity asks for to keep running, apart from building: its
        // production is scaled by the fraction of this alone (SIM-043).
        let mut upkeep: BTreeMap<EntityId, Request> = BTreeMap::new();
        for (id, e) in self.entities.iter().filter(|(_, e)| e.army == slot) {
            let Some(kind) = kind_of(world, &e.blueprint) else {
                continue;
            };
            let mut ask = Request::default();
            if e.complete {
                storage.mass = storage
                    .mass
                    .saturating_add(kind.storage_mass.unwrap_or(Q::ZERO));
                storage.energy = storage
                    .energy
                    .saturating_add(kind.storage_energy.unwrap_or(Q::ZERO));
                income.mass = income.mass.saturating_add(
                    per_tick(kind.production_mass.unwrap_or(Q::ZERO))
                        .saturating_mul(e.maintenance_fraction),
                );
                income.energy = income.energy.saturating_add(
                    per_tick(kind.production_energy.unwrap_or(Q::ZERO))
                        .saturating_mul(e.maintenance_fraction),
                );
                ask.energy = per_tick(kind.maintenance_energy.unwrap_or(Q::ZERO));
                upkeep.insert(
                    *id,
                    Request {
                        mass: Q::ZERO,
                        energy: ask.energy,
                    },
                );
            }
            let rates = e
                .work
                .filter(|_| e.complete)
                .and_then(|t| self.entities.get(&t))
                .filter(|t| !t.complete)
                .and_then(|t| kind_of(world, &t.blueprint))
                .zip(kind.build_rate)
                .and_then(|(target, rate)| build_rates(target, rate));
            if let Some(r) = rates {
                ask.mass = ask.mass.saturating_add(per_tick(r.mass_per_second));
                ask.energy = ask.energy.saturating_add(per_tick(r.energy_per_second));
            }
            if ask.mass > Q::ZERO || ask.energy > Q::ZERO {
                requests.insert(*id, ask);
            }
        }
        let Some(army) = self.armies.get_mut(&slot) else {
            return false;
        };
        let available_mass = army.mass.saturating_add(income.mass);
        let available_energy = army.energy.saturating_add(income.energy);
        let requested = requests
            .values()
            .fold(Request::default(), |sum, r| Request {
                mass: sum.mass.saturating_add(r.mass),
                energy: sum.energy.saturating_add(r.energy),
            });
        let fraction_mass = fraction_of(available_mass, requested.mass);
        let fraction_energy = fraction_of(available_energy, requested.energy);
        let mut granted = Request::default();
        let mut fractions: BTreeMap<EntityId, Q> = BTreeMap::new();
        for (id, r) in &requests {
            let f = r.fraction(fraction_mass, fraction_energy);
            granted.mass = granted.mass.saturating_add(f.saturating_mul(r.mass));
            granted.energy = granted.energy.saturating_add(f.saturating_mul(r.energy));
            fractions.insert(*id, f);
        }
        let settle = |available: Q, granted: Q, storage: Q| {
            available.saturating_sub(granted).max(Q::ZERO).min(storage)
        };
        army.mass = settle(available_mass, granted.mass, storage.mass);
        army.energy = settle(available_energy, granted.energy, storage.energy);
        army.mass_storage = storage.mass;
        army.energy_storage = storage.energy;
        army.ledger = Ledger {
            income_mass: income.mass,
            income_energy: income.energy,
            requested_mass: requested.mass,
            requested_energy: requested.energy,
            granted_mass: granted.mass,
            granted_energy: granted.energy,
            fraction_mass,
            fraction_energy,
        };
        for e in self.entities.values_mut().filter(|e| e.army == slot) {
            e.fraction = fractions.get(&e.id).copied().unwrap_or(Q::ONE);
            e.maintenance_fraction = upkeep
                .get(&e.id)
                .map_or(Q::ONE, |r| r.fraction(fraction_mass, fraction_energy));
        }
        fraction_mass < Q::ONE || fraction_energy < Q::ONE
    }
}
