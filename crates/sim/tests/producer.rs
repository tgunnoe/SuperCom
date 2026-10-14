//! A producer's output is scaled by the fraction granted to its maintenance
//! request alone (docs/specs/sim.md, SIM-043; docs/adr/0011-economy-and-build.md).
//! All synthetic; expected values are exact in fixed point and stated beside
//! each.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use supercom_sim::{Army, Entity, EntityId, Q, QueueMode, State};
use support::{
    build_at, builder, flat_map, frac, kind, order_cmd, pos, q, state, step, structure, world,
};

fn entity(s: &State, id: EntityId) -> &Entity {
    s.entities.get(&id).expect("the entity exists")
}

/// SIM-043: a commander that builds with mass short keeps its full income,
/// because its fraction of the build request does not scale its production;
/// a fabricator short of the energy it maintains on is scaled by that
/// shortage.
#[test]
fn sim_producer_scales_by_maintenance_fraction_only() {
    // A commander with 1 mass a tick of production and 1 energy a tick of
    // maintenance builds a generator that costs 2 mass a tick, from an empty
    // mass store and plentiful energy.
    let mut commander = builder("commander", 10, &["gen"]);
    commander.production_mass = Some(q(10));
    commander.maintenance_energy = Some(q(10));
    let w = world(
        vec![commander, structure("gen", 120, 600, 60)],
        Vec::new(),
        flat_map(32),
    );
    let mut s = state(&w, Q::ZERO, q(1_000_000));
    let b = s.spawn(Army(1), "commander", pos(4.5, 0.0, 4.5), q(100));
    step(
        &mut s,
        &[order_cmd(
            0,
            0,
            b,
            build_at("gen", 4.5, 8.5),
            QueueMode::Replace,
        )],
    );
    for _ in 0..3 {
        step(&mut s, &[]);
    }
    // Tick 3: the maintenance request is met in full, so the income is the
    // whole 1; the build request of 2 against available 1 is granted 1/2,
    // which scales the build and nothing else.
    let ledger = s.armies.get(&Army(1)).expect("army 1 exists").ledger;
    assert_eq!(ledger.income_mass, q(1));
    assert_eq!(ledger.fraction_mass, frac(1, 2));
    assert_eq!(entity(&s, b).fraction, frac(1, 2));
    assert_eq!(entity(&s, b).maintenance_fraction, Q::ONE);

    // A fabricator with 15 energy a tick of maintenance and 7.5 stored.
    let mut fab = kind("fab", &["STRUCTURE", "MASSFABRICATION"]);
    fab.maintenance_energy = Some(q(150));
    fab.production_mass = Some(q(1));
    fab.storage_mass = Some(q(100));
    fab.storage_energy = Some(q(100));
    let w = world(vec![fab], Vec::new(), flat_map(8));
    let mut s = state(&w, Q::ZERO, Q::from_bits(15 << 31));
    let id = s.spawn(Army(1), "fab", pos(1.5, 0.0, 1.5), q(100));
    step(&mut s, &[]);
    // 7.5 of 15 is granted: the maintenance fraction is 1/2 exactly.
    assert_eq!(entity(&s, id).maintenance_fraction, Q::from_bits(1 << 31));
    assert_eq!(entity(&s, id).fraction, Q::from_bits(1 << 31));
    step(&mut s, &[]);
    // Income is the production per tick, raw 429496729, times 1/2, floored.
    assert_eq!(
        s.armies
            .get(&Army(1))
            .expect("army 1")
            .ledger
            .income_mass
            .to_bits(),
        214_748_364
    );
    // The store is empty and stays so, so the fraction falls to 0 and the
    // income with it: the maintenance fraction tracks the request.
    assert_eq!(entity(&s, id).maintenance_fraction, Q::ZERO);
    step(&mut s, &[]);
    assert_eq!(
        s.armies.get(&Army(1)).expect("army 1").ledger.income_mass,
        Q::ZERO
    );
}
