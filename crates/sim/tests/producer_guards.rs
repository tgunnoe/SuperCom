//! A producer that makes no maintenance request is not scaled by the
//! energy shortage (docs/specs/sim.md, SIM-043): the fraction recorded for
//! an entity without an upkeep is 1, not the army's energy fraction. All
//! synthetic.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use supercom_sim::{Army, Q};
use support::{flat_map, kind, pos, q, state, step, world};

/// A mass producer with no maintenance beside a consumer of energy, from an
/// empty energy store: the consumer is starved (its fraction 0) and the
/// producer keeps its whole income.
#[test]
fn sim_a_producer_without_upkeep_ignores_the_energy_shortage() {
    let mut mine = kind("mine", &["STRUCTURE"]);
    mine.production_mass = Some(q(10));
    mine.storage_mass = Some(q(100));
    let mut pump = kind("pump", &["STRUCTURE"]);
    pump.maintenance_energy = Some(q(100));
    pump.storage_energy = Some(q(100));
    let w = world(vec![mine, pump], Vec::new(), flat_map(8));
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let m = s.spawn(Army(1), "mine", pos(1.5, 0.0, 1.5), q(100));
    let p = s.spawn(Army(1), "pump", pos(4.5, 0.0, 4.5), q(100));
    step(&mut s, &[]);
    step(&mut s, &[]);
    assert_eq!(
        s.entities.get(&p).expect("the pump").maintenance_fraction,
        Q::ZERO,
        "the pump is starved of energy"
    );
    assert_eq!(
        s.entities.get(&m).expect("the mine").maintenance_fraction,
        Q::ONE,
        "the mine made no maintenance request"
    );
    assert_eq!(
        s.armies.get(&Army(1)).expect("army 1").ledger.income_mass,
        q(1),
        "the mine's whole income, 10 a second, a tenth a tick"
    );
}
