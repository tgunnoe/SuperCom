//! Vision and victory rules on synthetic worlds (docs/specs/sim.md, SIM-083
//! to SIM-085; docs/adr/0014-vision-and-victory.md). No install, no file.
//! Every expected set, flag, and counter is stated beside its assertion
//! from the scenario's construction, never from the helpers under test.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::collections::BTreeSet;
use std::sync::Arc;

use supercom_sim::{
    Army, Command, EntityId, Order, Outcome, Q, QueueMode, State, Tick, UnitKind, World,
};
use support::{cats, flat_map, kind, pos, q, state, step, structure, world};

/// A complete-at-spawn unit that sees `radius` world units.
fn seer(id: &str, radius: f64) -> UnitKind {
    let mut k = kind(id, &["MOBILE"]);
    k.vision_radius = Some(radius);
    k
}

fn plain_world(units: Vec<UnitKind>) -> Arc<World> {
    world(units, Vec::new(), flat_map(128))
}

fn set(ids: &[EntityId]) -> BTreeSet<EntityId> {
    ids.iter().copied().collect()
}

/// A state with armies 1 to `armies`, each owning a commander at its own
/// corner, far from every other unit; the ids are returned in army order.
fn commanded(armies: u8) -> (State, Vec<EntityId>) {
    let w = plain_world(vec![
        kind("cmd", &["MOBILE", "COMMAND"]),
        kind("grunt", &["MOBILE"]),
    ]);
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let mut ids = Vec::new();
    for n in 1..=armies {
        if n > 1 {
            s.add_army(Army(n), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
        }
        ids.push(s.spawn(Army(n), "cmd", pos(f64::from(n) * 20.0, 0.0, 5.0), q(100)));
    }
    (s, ids)
}

fn kill(s: &mut State, id: EntityId) {
    s.entities.get_mut(&id).expect("the entity exists").health = Q::ZERO;
}

fn move_cmd(tick: u64, seq: u32, army: u8, unit: EntityId) -> Command {
    Command {
        tick: Tick(tick),
        source: 0,
        seq,
        army: Army(army),
        units: vec![unit],
        order: Order::Move {
            to: pos(10.0, 0.0, 10.0),
        },
        mode: QueueMode::Replace,
    }
}

/// SIM-083: vision is the kind's radius on the horizontal plane, inclusive.
#[test]
fn sim_vision_sets_from_vision_radius() {
    let mut site = structure("site", 1, 1, 10);
    site.vision_radius = Some(500.0);
    let w = plain_world(vec![seer("eye", 10.0), kind("blind", &["MOBILE"]), site]);
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    s.add_army(Army(2), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);

    let eye = s.spawn(Army(1), "eye", pos(50.0, 0.0, 50.0), q(100));
    let own_far = s.spawn(Army(1), "blind", pos(120.0, 0.0, 120.0), q(100));
    // Distances from the eye: 9.5, 10.0 exactly, 10.5, and the diagonal
    // (7, 7), which is 9.899 and so inside, and (7.2, 7.2), 10.182, outside;
    // a Chebyshev or Manhattan reading would get the last two wrong. A
    // height difference does not count.
    let near = s.spawn(Army(2), "blind", pos(59.5, 0.0, 50.0), q(100));
    let edge = s.spawn(Army(2), "blind", pos(50.0, 0.0, 60.0), q(100));
    let far = s.spawn(Army(2), "blind", pos(60.5, 0.0, 50.0), q(100));
    let diag_in = s.spawn(Army(2), "blind", pos(57.0, 0.0, 57.0), q(100));
    let diag_out = s.spawn(Army(2), "blind", pos(57.2, 0.0, 57.2), q(100));
    let high = s.spawn(Army(2), "blind", pos(50.0, 90.0, 55.0), q(100));
    // An unfinished seeing site, even with a huge radius, sees nothing.
    let under = s
        .spawn_under_construction(Army(2), "site", pos(50.0, 0.0, 90.0))
        .expect("a buildable kind");
    step(&mut s, &[]);

    let a1 = set(&[eye, own_far, near, edge, diag_in, high]);
    assert_eq!(s.visible(Army(1)), &a1);
    for id in [far, diag_out, under] {
        assert!(!s.is_visible(Army(1), id), "{id:?} is out of range");
    }
    for id in &a1 {
        assert!(s.is_visible(Army(1), *id));
    }
    // Army 2 sees only its own: its seeing site is unfinished and its other
    // units have no radius, so it sees none of army 1's.
    let a2 = set(&[near, edge, far, diag_in, diag_out, high, under]);
    assert_eq!(s.visible(Army(2)), &a2);
    assert!(!s.is_visible(Army(2), eye));
    assert!(
        s.visible(Army(9)).is_empty(),
        "an unknown army sees nothing"
    );
    let snap = s.snapshot();
    let seen_by = |id| {
        snap.entities
            .iter()
            .find(|e| e.id == id)
            .expect("the entity")
            .seen_by
            .clone()
    };
    assert_eq!(seen_by(eye), vec![Army(1)]);
    assert_eq!(seen_by(near), vec![Army(1), Army(2)]);
    assert_eq!(seen_by(far), vec![Army(2)]);

    // The sets are rebuilt each phase: out of range after a move, back in
    // range after another; a seer finished later sees from then on.
    s.entities.get_mut(&near).expect("near").pos = pos(80.0, 0.0, 50.0);
    step(&mut s, &[]);
    assert!(!s.is_visible(Army(1), near));
    assert!(s.is_visible(Army(2), near));
    s.entities.get_mut(&near).expect("near").pos = pos(50.0, 0.0, 41.0);
    step(&mut s, &[]);
    assert!(s.is_visible(Army(1), near));
    let site = s.entities.get_mut(&under).expect("site");
    (site.complete, site.health) = (true, q(100));
    step(&mut s, &[]);
    assert!(s.is_visible(Army(2), eye), "a finished site sees 500 units");

    // A unit that dies is gone from every set the same tick.
    kill(&mut s, near);
    step(&mut s, &[]);
    assert!(!s.is_visible(Army(1), near));
    assert!(!s.is_visible(Army(2), near));
}

/// SIM-084: demoralization, from the commanders that exist and are finished.
#[test]
fn sim_army_defeated_without_a_commander() {
    let (mut s, ids) = commanded(2);
    kill(&mut s, ids[1]);
    step(&mut s, &[]);
    assert_eq!(s.counters.deaths, 1);
    assert!(
        s.armies[&Army(2)].defeated,
        "defeated in the tick of the kill"
    );
    assert!(!s.armies[&Army(1)].defeated);
    assert_eq!(s.counters.defeats, 1);
    assert_eq!(s.outcome, Some(Outcome::Winner(Army(1))));
    assert_eq!(s.counters.victories, 1);
    assert_eq!(s.snapshot().outcome, Some(Outcome::Winner(Army(1))));
    assert!(
        s.snapshot()
            .armies
            .iter()
            .any(|a| a.id == Army(2) && a.defeated)
    );

    // A third army with a grunt and no commander is defeated on the first
    // tick; its grunt keeps existing, and its Move is ignored and counted
    // from the next tick, while a commanded army's Move is applied.
    let (mut s, ids) = commanded(3);
    let grunt = s.spawn(Army(3), "grunt", pos(100.0, 0.0, 100.0), q(100));
    kill(&mut s, ids[2]);
    step(&mut s, &[]);
    assert!(s.armies[&Army(3)].defeated);
    assert_eq!(s.counters.defeats, 1);
    assert_eq!(s.outcome, None, "two armies still stand");
    assert!(s.entities.contains_key(&grunt), "a defeat removes nothing");
    let before = (s.counters.commands_applied, s.counters.commands_ignored);
    step(
        &mut s,
        &[move_cmd(1, 0, 3, grunt), move_cmd(1, 1, 1, ids[0])],
    );
    assert_eq!(
        (s.counters.commands_applied, s.counters.commands_ignored),
        (before.0 + 1, before.1 + 1)
    );
    assert!(s.entities[&grunt].orders.is_empty());
    assert_eq!(s.entities[&ids[0]].orders.len(), 1);
    assert_eq!(
        s.counters.defeats, 1,
        "a defeated army's later tick is not counted again"
    );

    // A defeat clears no order: a Move stamped in the tick of the loss is
    // applied before the phase that defeats the army, and stays held.
    let (mut s, ids) = commanded(3);
    let grunt = s.spawn(Army(3), "grunt", pos(100.0, 0.0, 100.0), q(100));
    kill(&mut s, ids[2]);
    step(&mut s, &[move_cmd(0, 0, 3, grunt)]);
    assert!(s.armies[&Army(3)].defeated);
    assert_eq!(s.entities[&grunt].orders.len(), 1, "the order is held");

    // A commander that is still a site does not keep its army in the match.
    let mut site = structure("csite", 1, 1, 10);
    site.categories = cats(&["COMMAND"]);
    let w = plain_world(vec![kind("cmd", &["COMMAND"]), site]);
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    s.add_army(Army(2), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
    s.spawn(Army(1), "cmd", pos(5.0, 0.0, 5.0), q(100));
    s.spawn_under_construction(Army(2), "csite", pos(9.0, 0.0, 9.0))
        .expect("a buildable kind");
    step(&mut s, &[]);
    assert!(s.armies[&Army(2)].defeated);
    assert_eq!(s.outcome, Some(Outcome::Winner(Army(1))));

    // A world with no COMMAND kind has no commanders to lose, so it defeats
    // nobody (SIM-084); the synthetic worlds of the other families rely on
    // it, and it is checked after a defeat above so a no-op cannot pass.
    let w = plain_world(vec![kind("grunt", &["MOBILE"])]);
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    s.add_army(Army(2), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
    s.spawn(Army(1), "grunt", pos(5.0, 0.0, 5.0), q(100));
    step(&mut s, &[]);
    assert!(s.armies.values().all(|a| !a.defeated));
    assert_eq!(
        (s.outcome, s.counters.defeats, s.counters.victories),
        (None, 0, 0)
    );
}

/// SIM-085: one outcome, decided once.
#[test]
fn sim_last_army_standing_wins_once() {
    // Two of three commanders die in one tick.
    let (mut s, ids) = commanded(3);
    kill(&mut s, ids[0]);
    kill(&mut s, ids[1]);
    step(&mut s, &[]);
    assert_eq!(s.outcome, Some(Outcome::Winner(Army(3))));
    assert_eq!((s.counters.defeats, s.counters.victories), (2, 1));
    // After the outcome nothing changes: the winner's own commander dies,
    // and the ticks run on.
    let frozen = (s.outcome, s.counters.defeats, s.counters.victories);
    kill(&mut s, ids[2]);
    for _ in 0..3 {
        step(&mut s, &[]);
    }
    assert_eq!(s.counters.deaths, 3, "the death still happens");
    assert_eq!(
        (s.outcome, s.counters.defeats, s.counters.victories),
        frozen
    );
    assert!(
        !s.armies[&Army(3)].defeated,
        "the decided match stops judging"
    );

    // All commanders die in one tick: a draw, no victory.
    let (mut s, ids) = commanded(3);
    for id in ids {
        kill(&mut s, id);
    }
    step(&mut s, &[]);
    assert_eq!(s.outcome, Some(Outcome::Draw));
    assert_eq!((s.counters.defeats, s.counters.victories), (3, 0));
    step(&mut s, &[]);
    assert_eq!(s.outcome, Some(Outcome::Draw));

    // Nobody wins while two stand.
    let (mut s, ids) = commanded(3);
    kill(&mut s, ids[0]);
    step(&mut s, &[]);
    assert_eq!(s.outcome, None);
    assert_eq!((s.counters.defeats, s.counters.victories), (1, 0));

    // A lone army, commanded or not, and an empty state decide nothing.
    let (mut s, _) = commanded(1);
    step(&mut s, &[]);
    assert_eq!(
        (s.outcome, s.counters.defeats, s.counters.victories),
        (None, 0, 0)
    );
    let mut empty = State::with_world(plain_world(vec![kind("cmd", &["COMMAND"])]), 1);
    step(&mut empty, &[]);
    assert_eq!(empty.outcome, None);
    let w = plain_world(vec![kind("cmd", &["COMMAND"])]);
    let mut lone = state(&w, Q::ZERO, Q::ZERO);
    step(&mut lone, &[]);
    assert!(
        !lone.armies[&Army(1)].defeated,
        "a lone army is not defeated"
    );
    assert_eq!((lone.outcome, lone.counters.defeats), (None, 0));
}
