//! A command that replaces a queue also ends the work the old head
//! explained, and a builder whose target is gone retries its head order
//! (docs/specs/sim.md, SIM-059 to SIM-061). All synthetic.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::sync::Arc;

use supercom_sim::{Army, Entity, EntityId, Order, Q, QueueMode, State, World};
use support::{
    build_at, builder, cats, flat_map, order_cmd, pos, q, state, step, structure, world,
};

fn entity(s: &State, id: EntityId) -> &Entity {
    s.entities.get(&id).expect("the entity exists")
}

fn requested_mass(s: &State) -> Q {
    s.armies
        .get(&Army(1))
        .expect("army 1 exists")
        .ledger
        .requested_mass
}

/// A worker of rate 10 that builds `gen` (build time 60, cost 60 mass: 1
/// per tick at rate 10), and a factory of rate 10 that builds `tank` and
/// `jeep` (each the same cost and time).
fn rich_world() -> Arc<World> {
    let mut factory = builder("factory", 10, &["tank", "jeep"]);
    factory.categories = cats(&["STRUCTURE", "FACTORY"]);
    let mut tank = structure("tank", 60, 600, 60);
    tank.categories = cats(&["MOBILE"]);
    let mut jeep = structure("jeep", 60, 600, 60);
    jeep.categories = cats(&["MOBILE"]);
    world(
        vec![
            builder("worker", 10, &["gen"]),
            structure("gen", 60, 600, 60),
            factory,
            tank,
            jeep,
        ],
        Vec::new(),
        flat_map(32),
    )
}

fn rich(w: &Arc<World>) -> State {
    state(w, q(1_000_000), q(1_000_000))
}

/// A Replace to a working structure builder: in that tick the old site
/// takes no work and is charged nothing, it stays where it is, and the new
/// site spawns in the build phase.
#[test]
fn replace_to_a_working_builder_starts_the_new_site() {
    let w = rich_world();
    let mut s = rich(&w);
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
    for _ in 1..10 {
        step(&mut s, &[]);
    }
    let old = EntityId(2);
    let before = entity(&s, old).built;
    assert!(before > Q::ZERO, "the old site is mid-build");

    step(
        &mut s,
        &[order_cmd(
            10,
            1,
            b,
            build_at("gen", 8.5, 8.5),
            QueueMode::Replace,
        )],
    );
    assert_eq!(entity(&s, old).built, before, "the old site gains nothing");
    assert!(!entity(&s, old).complete);
    assert_eq!(requested_mass(&s), Q::ZERO, "the old target is not charged");
    let new = EntityId(3);
    assert_eq!(entity(&s, new).pos.x, 8.5, "the new site spawned this tick");
    assert_eq!(entity(&s, b).work, Some(new));
}

/// The same for a factory: the replaced unit stops growing and the new
/// one spawns in that build phase.
#[test]
fn replace_to_a_working_factory_starts_the_new_unit() {
    let w = rich_world();
    let mut s = rich(&w);
    let f = s.spawn(Army(1), "factory", pos(10.5, 0.0, 10.5), q(100));
    step(
        &mut s,
        &[order_cmd(
            0,
            0,
            f,
            Order::BuildFactory {
                blueprint: "tank".to_owned(),
            },
            QueueMode::Replace,
        )],
    );
    for _ in 1..10 {
        step(&mut s, &[]);
    }
    let old = EntityId(2);
    let before = entity(&s, old).built;
    assert!(before > Q::ZERO, "the tank is mid-build");

    step(
        &mut s,
        &[order_cmd(
            10,
            1,
            f,
            Order::BuildFactory {
                blueprint: "jeep".to_owned(),
            },
            QueueMode::Replace,
        )],
    );
    assert_eq!(entity(&s, old).built, before, "the tank gains nothing");
    assert_eq!(requested_mass(&s), Q::ZERO, "the tank is not charged");
    let new = entity(&s, EntityId(3));
    assert_eq!(new.blueprint, "jeep", "the jeep spawned this tick");
    assert_eq!(entity(&s, f).work, Some(new.id));
}

/// A builder whose site is removed retries its head order instead of
/// waiting on a target that no longer exists.
#[test]
fn builder_whose_target_is_gone_retries_its_head_order() {
    let w = rich_world();
    let mut s = rich(&w);
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
    for _ in 1..5 {
        step(&mut s, &[]);
    }
    s.entities.remove(&EntityId(2));
    step(&mut s, &[]);
    let again = EntityId(3);
    assert_eq!(entity(&s, again).pos.z, 8.5, "the site spawned again");
    assert_eq!(entity(&s, b).work, Some(again));
}
