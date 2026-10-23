//! Only complete entities act: an unfinished one starts no build, takes no
//! Guard assignment, adds no work, and requests nothing, while commands
//! naming it still queue (docs/specs/sim.md, SIM-045, SIM-059, SIM-061).
//! All synthetic.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::sync::Arc;

use supercom_sim::{Army, Entity, EntityId, Order, Q, QueueMode, State, World};
use support::{
    build_at, builder, flat_map, kind, order_cmd, pos, q, state, step, structure, world,
};

fn entity(s: &State, id: EntityId) -> &Entity {
    s.entities.get(&id).expect("the entity exists")
}

/// A builder kind that can itself be built: it carries a build time.
fn buildable_builder(id: &str, rate: i64, buildable: &[&str]) -> supercom_sim::UnitKind {
    let mut k = builder(id, rate, buildable);
    k.build_time = Some(q(600));
    k.build_cost_mass = q(10);
    k.build_cost_energy = q(10);
    k
}

fn rich_world() -> Arc<World> {
    world(
        vec![
            buildable_builder("worker", 10, &["gen"]),
            buildable_builder("helper", 5, &[]),
            structure("gen", 60, 600, 60),
            kind("nobuildtime", &["STRUCTURE"]),
        ],
        Vec::new(),
        flat_map(32),
    )
}

fn rich(w: &Arc<World>) -> State {
    state(w, q(1_000_000), q(1_000_000))
}

fn requested(s: &State) -> (Q, Q) {
    let l = s.armies.get(&Army(1)).expect("army 1 exists").ledger;
    (l.requested_mass, l.requested_energy)
}

/// A kind without a build time cannot be under construction.
#[test]
fn a_kind_without_a_build_time_cannot_be_spawned_unfinished() {
    let w = rich_world();
    let mut s = rich(&w);
    assert_eq!(
        s.spawn_under_construction(Army(1), "nobuildtime", pos(1.5, 0.0, 1.5)),
        None
    );
    assert!(s.entities.is_empty(), "nothing spawned");
    assert_eq!(s.next_id, EntityId(1), "no id was spent");
}

/// An unfinished worker accepts the order, spawns nothing and requests
/// nothing until it completes, then starts the build.
#[test]
fn an_unfinished_builder_waits_with_its_order_until_complete() {
    let w = rich_world();
    let mut s = rich(&w);
    let b = s
        .spawn_under_construction(Army(1), "worker", pos(4.5, 0.0, 4.5))
        .expect("the worker has a build time");
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
    assert_eq!(s.counters.commands_applied, 1, "the command is accepted");
    for _ in 0..5 {
        step(&mut s, &[]);
    }
    assert_eq!(s.entities.len(), 1, "no site was spawned");
    assert_eq!(entity(&s, b).work, None);
    assert_eq!(entity(&s, b).orders.len(), 1, "the order waits");
    assert_eq!(requested(&s), (Q::ZERO, Q::ZERO));

    let worker = s.entities.get_mut(&b).expect("the worker exists");
    worker.complete = true;
    worker.health = worker.max_health;
    step(&mut s, &[]);
    assert_eq!(s.entities.len(), 2, "the site spawns once complete");
    assert_eq!(entity(&s, b).work, Some(EntityId(2)));
}

/// An unfinished assister takes no assignment and contributes nothing.
#[test]
fn an_unfinished_assister_contributes_nothing() {
    let w = rich_world();
    let mut s = rich(&w);
    let b = s.spawn(Army(1), "worker", pos(4.5, 0.0, 4.5), q(100));
    let h = s
        .spawn_under_construction(Army(1), "helper", pos(5.5, 0.0, 4.5))
        .expect("the helper has a build time");
    step(
        &mut s,
        &[
            order_cmd(0, 0, b, build_at("gen", 4.5, 8.5), QueueMode::Replace),
            order_cmd(0, 1, h, Order::Guard { target: b }, QueueMode::Replace),
        ],
    );
    for _ in 0..10 {
        step(&mut s, &[]);
    }
    let site = EntityId(3);
    // The worker alone: ten increments of per_tick(10) = 1.
    assert_eq!(entity(&s, site).built, q(10));
    assert_eq!(entity(&s, h).work, None);
}
