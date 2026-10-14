//! Guards on the economy phase (docs/specs/sim.md, SIM-040, SIM-043,
//! SIM-059): the granted fraction scales work, the stall counter counts
//! ticks, and a fraction does not outlive its request. All synthetic;
//! expected values are exact in fixed point and stated beside each.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::sync::Arc;

use supercom_sim::{Army, Command, Entity, EntityId, Order, Q, QueueMode, State, Tick, World};
use support::{
    build_at, builder, flat_map, frac, order_cmd, pos, q, state, step, structure, world,
};

fn entity(s: &State, id: EntityId) -> &Entity {
    s.entities.get(&id).expect("the entity exists")
}

/// A worker of rate 10 building `gen`: build time 60, so 6 seconds, and a
/// cost of 60 mass and 600 energy, which is 1 mass and 10 energy per tick.
fn gen_world() -> Arc<World> {
    world(
        vec![
            builder("worker", 10, &["gen"]),
            structure("gen", 60, 600, 60),
        ],
        Vec::new(),
        flat_map(32),
    )
}

/// A stalled builder gains its granted fraction times its rate per tick.
#[test]
fn stalled_builder_gains_fraction_times_rate() {
    let w = gen_world();
    // Half a unit of mass: the first request of 1 per tick is covered 1/2.
    let mut s = state(&w, frac(1, 2), q(1_000_000));
    let b = s.spawn(Army(1), "worker", pos(4.5, 0.0, 4.5), q(100));
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
    let site = EntityId(2);
    assert_eq!(entity(&s, site).built, Q::ZERO, "tick 0 only starts it");
    step(&mut s, &[]);
    // Tick 1: available 1/2, requested 1, fraction 1/2, gain 1/2 * 1.
    assert_eq!(entity(&s, b).fraction, frac(1, 2));
    assert_eq!(entity(&s, site).built, frac(1, 2));
    step(&mut s, &[]);
    // Tick 2: nothing is left, the fraction is 0 and the site gains nothing.
    assert_eq!(entity(&s, b).fraction, Q::ZERO);
    assert_eq!(entity(&s, site).built, frac(1, 2));
}

/// Two armies stalling in one tick add one stall tick, not two.
#[test]
fn two_stalled_armies_count_one_stall_tick() {
    let w = gen_world();
    let mut s = state(&w, frac(1, 2), q(1_000_000));
    s.add_army(Army(2), frac(1, 2), q(1_000_000), Q::ZERO, Q::ZERO);
    let first = s.spawn(Army(1), "worker", pos(4.5, 0.0, 4.5), q(100));
    let second = s.spawn(Army(2), "worker", pos(20.5, 0.0, 4.5), q(100));
    let second_cmd = Command {
        tick: Tick(0),
        source: 0,
        seq: 1,
        army: Army(2),
        units: vec![second],
        order: build_at("gen", 20.5, 8.5),
        mode: QueueMode::Replace,
    };
    step(
        &mut s,
        &[
            order_cmd(0, 0, first, build_at("gen", 4.5, 8.5), QueueMode::Replace),
            second_cmd,
        ],
    );
    assert_eq!(s.counters.stall_ticks, 0, "nothing was requested at tick 0");
    step(&mut s, &[]);
    assert_eq!(entity(&s, first).fraction, frac(1, 2));
    assert_eq!(entity(&s, second).fraction, frac(1, 2));
    assert_eq!(s.counters.stall_ticks, 1);
}

/// An entity that stops requesting has fraction 1 at the next settlement.
#[test]
fn a_stopped_builder_returns_to_fraction_one() {
    let w = gen_world();
    let mut s = state(&w, frac(1, 2), q(1_000_000));
    let b = s.spawn(Army(1), "worker", pos(4.5, 0.0, 4.5), q(100));
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
    step(&mut s, &[]);
    assert_eq!(entity(&s, b).fraction, frac(1, 2), "stalled at tick 1");
    step(
        &mut s,
        &[order_cmd(2, 1, b, Order::Stop, QueueMode::Replace)],
    );
    assert_eq!(entity(&s, b).fraction, Q::ONE, "its request ended");
}
