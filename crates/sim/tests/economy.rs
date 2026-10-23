//! Acceptance tests of the economy rules (docs/specs/sim.md, SIM-040,
//! SIM-043, SIM-057, SIM-058). All synthetic. Fixed-point values are stated
//! as raw bits computed by the integer arithmetic shown beside them.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use supercom_sim::{Army, Entity, EntityId, Order, Q, State, per_tick};
use support::{builder, flat_map, frac, kind, pos, q, state, step, structure, world};

/// The economy's view of an army after a step.
fn army(s: &State) -> &supercom_sim::ArmyState {
    s.armies.get(&Army(1)).expect("army 1 exists")
}

fn entity(s: &State, id: EntityId) -> &Entity {
    s.entities.get(&id).expect("the entity exists")
}

/// SIM-057: a per-second rate applies per tick as the value divided by 10,
/// by integer division of the raw bits (truncating toward zero).
#[test]
fn sim_economy_rates_apply_per_tick_as_a_tenth() {
    // 6 << 32 = 25769803776; / 10 = 2576980377 (truncated from .6).
    assert_eq!(per_tick(q(6)).to_bits(), 2_576_980_377);
    // 1 << 32 = 4294967296; / 10 = 429496729 (truncated from .6); one raw
    // unit below the nearest Q to a tenth, which is 429496730.
    assert_eq!(per_tick(q(1)).to_bits(), 429_496_729);
    // 0.3 as raw bits: (3 << 32) / 10 = 1288490188; / 10 = 128849018.
    assert_eq!(per_tick(frac(3, 10)).to_bits(), 128_849_018);
    // Exact where the division is exact.
    assert_eq!(per_tick(q(10)), q(1));
    assert_eq!(per_tick(q(20)), q(2));
    assert_eq!(per_tick(Q::ZERO), Q::ZERO);
}

/// SIM-058: storage is the sum of complete units, income is each producer's
/// production per tick times its last fraction, the stored amount is clamped
/// to storage, and the ledger records the settlement.
#[test]
fn sim_income_and_storage_settle_each_tick() {
    let mut commander = kind("commander", &["COMMAND"]);
    commander.production_mass = Some(q(1));
    commander.production_energy = Some(q(20));
    commander.storage_mass = Some(q(650));
    commander.storage_energy = Some(q(4000));
    let mut depot = structure("depot", 10, 10, 100);
    depot.storage_mass = Some(q(500));
    let w = world(vec![commander, depot], Vec::new(), flat_map(8));
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    s.spawn(Army(1), "commander", pos(1.5, 0.0, 1.5), q(100));

    step(&mut s, &[]);
    // Income is raw (1 << 32) / 10 = 429496729 mass and (20 << 32) / 10 =
    // 2 << 32 energy; nothing is requested, so nothing is granted.
    assert_eq!(army(&s).mass.to_bits(), 429_496_729);
    assert_eq!(army(&s).energy, q(2));
    assert_eq!(army(&s).mass_storage, q(650));
    assert_eq!(army(&s).energy_storage, q(4000));
    let l = army(&s).ledger;
    assert_eq!(l.income_mass.to_bits(), 429_496_729);
    assert_eq!(l.income_energy, q(2));
    assert_eq!((l.requested_mass, l.requested_energy), (Q::ZERO, Q::ZERO));
    assert_eq!((l.granted_mass, l.granted_energy), (Q::ZERO, Q::ZERO));
    assert_eq!((l.fraction_mass, l.fraction_energy), (Q::ONE, Q::ONE));

    // A depot under construction adds no storage; a complete one adds 500.
    s.spawn_under_construction(Army(1), "DEPOT", pos(4.5, 0.0, 4.5))
        .expect("the depot kind exists");
    step(&mut s, &[]);
    assert_eq!(army(&s).mass_storage, q(650));
    s.spawn(Army(1), "depot", pos(6.5, 0.0, 6.5), q(100));
    step(&mut s, &[]);
    assert_eq!(army(&s).mass_storage, q(1150));
    assert_eq!(army(&s).energy_storage, q(4000));

    // The clamp: just under the storage plus one tick of income stops at it.
    s.armies.get_mut(&Army(1)).expect("army 1").mass = q(1150) - Q::from_bits(1);
    step(&mut s, &[]);
    assert_eq!(army(&s).mass, q(1150));
}

/// A builder of rate 10 at the origin of a 8x8 map building `target`, its
/// work item written directly so the economy sees the request.
fn working_builder(s: &mut State, at: f64, target_kind: &str) -> (EntityId, EntityId) {
    let b = s.spawn(Army(1), "worker", pos(at, 0.0, 0.5), q(100));
    let t = s
        .spawn_under_construction(Army(1), target_kind, pos(at, 0.0, 2.5))
        .expect("the target kind exists");
    let e = s.entities.get_mut(&b).expect("the builder");
    e.work = Some(t);
    e.orders.push_back(Order::BuildMobile {
        blueprint: target_kind.to_owned(),
        at: pos(at, 0.0, 2.5),
    });
    (b, t)
}

/// SIM-040: with stored 10 mass and no income, two builders each requesting
/// 8 are granted the fraction 10/16 of it; a unit short of one resource
/// consumes the other at the same fraction; an energy-only consumer is
/// granted in full; the stored amount reaches 0; the stall is counted.
#[test]
fn sim_stall_grants_fraction_and_clamps_storage() {
    // Rate 10 on build time 10 is 1 second; mass cost 80 is 80 per second,
    // 8 per tick; energy cost 100 is 10 per tick.
    let worker = builder("worker", 10, &["thing"]);
    let thing = structure("thing", 80, 100, 10);
    let mut pump = kind("pump", &["STRUCTURE"]);
    pump.maintenance_energy = Some(q(30)); // 3 per tick
    let w = world(vec![worker, thing, pump], Vec::new(), flat_map(8));
    let mut s = state(&w, q(10), q(1000));
    working_builder(&mut s, 1.5, "thing");
    working_builder(&mut s, 4.5, "thing");
    s.spawn(Army(1), "pump", pos(7.5, 0.0, 7.5), q(100));

    step(&mut s, &[]);
    // Fraction 10 / 16 = 0.625, raw (10 << 32 << 32) / (16 << 32) =
    // 5 << 29; each builder is granted 0.625 * 8 = 5 mass and 6.25 energy.
    let f = Q::from_bits(5 << 29);
    let a = army(&s);
    assert_eq!(a.ledger.requested_mass, q(16));
    assert_eq!(a.ledger.requested_energy, q(23)); // 10 + 10 + 3
    assert_eq!(a.ledger.fraction_mass, f);
    assert_eq!(a.ledger.fraction_energy, Q::ONE);
    assert_eq!(a.ledger.granted_mass, q(10));
    assert_eq!(a.ledger.granted_energy, Q::from_bits(31 << 31)); // 15.5
    assert_eq!(a.mass, Q::ZERO);
    assert_eq!(a.energy, Q::from_bits(1969 << 31)); // 984.5
    assert_eq!(s.counters.stall_ticks, 1);
    for id in [EntityId(1), EntityId(3)] {
        assert_eq!(entity(&s, id).fraction, f);
    }
    let pump_id = EntityId(5);
    assert_eq!(entity(&s, pump_id).fraction, Q::ONE);

    // The next tick starts from nothing: fraction 0 for the mass-short
    // builders; the pump is still granted in full.
    step(&mut s, &[]);
    let a = army(&s);
    assert_eq!(a.ledger.fraction_mass, Q::ZERO);
    assert_eq!(a.ledger.granted_mass, Q::ZERO);
    assert_eq!(a.ledger.granted_energy, q(3));
    assert_eq!(s.counters.stall_ticks, 2);
    assert_eq!(entity(&s, EntityId(1)).fraction, Q::ZERO);

    // A tick with no shortage does not add a stall.
    s.armies.get_mut(&Army(1)).expect("army 1").mass = q(1000);
    step(&mut s, &[]);
    assert_eq!(army(&s).ledger.fraction_mass, Q::ONE);
    assert_eq!(s.counters.stall_ticks, 2);
}

/// SIM-043: a unit's production is multiplied by the fraction granted to its
/// maintenance request alone in the last economy phase, 1 when it makes none,
/// and a fabricator keeps requesting its maintenance energy under a
/// shortage.
#[test]
fn sim_mass_fabricator_scales_with_energy_fraction() {
    let mut fab = kind("fab", &["STRUCTURE", "MASSFABRICATION"]);
    fab.maintenance_energy = Some(q(150)); // 15 per tick
    fab.production_mass = Some(q(1));
    fab.storage_mass = Some(q(100));
    fab.storage_energy = Some(q(100));
    let w = world(vec![fab], Vec::new(), flat_map(8));
    // 7.5 energy: raw 15 << 31.
    let mut s = state(&w, Q::ZERO, Q::from_bits(15 << 31));
    let id = s.spawn(Army(1), "fab", pos(1.5, 0.0, 1.5), q(100));

    step(&mut s, &[]);
    // The first tick's income uses the initial fraction 1: raw 429496729.
    assert_eq!(army(&s).ledger.income_mass.to_bits(), 429_496_729);
    assert_eq!(army(&s).ledger.requested_energy, q(15));
    // 7.5 / 15 = 0.5 exactly.
    assert_eq!(entity(&s, id).fraction, Q::from_bits(1 << 31));
    assert_eq!(army(&s).energy, Q::ZERO);

    step(&mut s, &[]);
    // Income is the production per tick times the last fraction 0.5:
    // 429496729 * 0.5 = 214748364 (floored); the request is unchanged.
    assert_eq!(army(&s).ledger.income_mass.to_bits(), 214_748_364);
    assert_eq!(army(&s).ledger.requested_energy, q(15));
    assert_eq!(entity(&s, id).fraction, Q::ZERO);

    step(&mut s, &[]);
    assert_eq!(army(&s).ledger.income_mass, Q::ZERO);
    assert_eq!(army(&s).ledger.requested_energy, q(15));
}
