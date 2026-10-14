//! A guard plans once per change of its target's cell (docs/specs/sim.md,
//! SIM-068): the cell it last planned for is kept in its goal, so a guard
//! of a target that stays put, reachable or not, searches once. The
//! planner-call counter `State::plans` is a diagnostic outside the
//! serialized state. All synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::collections::BTreeMap;

use supercom_sim::{
    Army, EntityId, FOLLOW_RANGE, Layer, MapGrid, MotionType, Order, Q, QueueMode, State, UnitKind,
    cell_of,
};
use support::{kind, order_cmd, pos, q, state, step, world};

fn mobile(id: &str, motion: MotionType, speed: f64) -> UnitKind {
    let mut k = kind(id, &["MOBILE"]);
    k.motion = motion;
    k.max_speed = Some(speed);
    k
}

/// A 64 by 64 land grid with the column `x == 32` impassable on land when
/// `walled`, which splits the grid into two components.
fn grid(walled: bool) -> MapGrid {
    let size = 64_u32;
    let cells = (size * size) as usize;
    let land = (0..cells)
        .map(|i| !(walled && i as u32 % size == 32))
        .collect();
    let mut passable = BTreeMap::new();
    passable.insert(Layer::Land, land);
    passable.insert(Layer::Air, vec![true; cells]);
    MapGrid {
        width: size,
        height: size,
        heights: vec![Q::ZERO; ((size + 1) * (size + 1)) as usize],
        passable,
        water: None,
    }
}

fn dist2(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)
}

/// A flying target moves slowly along a row and a guard walks after it,
/// out of range throughout. The planner is called once for the target's
/// Move and then by the guard exactly when it starts out of range and
/// whenever the target's cell, as the guard sees it after the target's own
/// move, differs from the cell last planned for; the target is spawned
/// first so it moves first in the phase.
fn count_plans(walled: bool, guard_at: (f64, f64), target_at: (f64, f64), goal_x: f64) {
    let w = world(
        vec![
            mobile("flyer", MotionType::Air, 0.5),
            mobile("walker", MotionType::Land, 1.0),
        ],
        Vec::new(),
        grid(walled),
    );
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let target = s.spawn(Army(1), "flyer", pos(target_at.0, 0.0, target_at.1), q(100));
    let guard = s.spawn(Army(1), "walker", pos(guard_at.0, 0.0, guard_at.1), q(100));
    let mut expected = 1_u64;
    let mut planned_for: Option<(u32, u32)> = None;
    let mut changes = 0;
    for tick in 0..100_u64 {
        let before = s.entities.get(&guard).expect("the guard").pos;
        let cmds = if tick == 0 {
            vec![
                order_cmd(
                    0,
                    0,
                    target,
                    Order::Move {
                        to: pos(goal_x, 0.0, target_at.1),
                    },
                    QueueMode::Replace,
                ),
                order_cmd(0, 1, guard, Order::Guard { target }, QueueMode::Replace),
            ]
        } else {
            Vec::new()
        };
        step(&mut s, &cmds);
        let seen = s.entities.get(&target).expect("the target").pos;
        assert!(
            dist2((before.x, before.z), (seen.x, seen.z)) > FOLLOW_RANGE * FOLLOW_RANGE,
            "the guard stays out of range at tick {tick}"
        );
        let cell = cell_of(&seen);
        if cell != planned_for {
            expected += 1;
            changes += 1;
            planned_for = cell;
        }
        assert_eq!(s.plans, expected, "planner calls after tick {tick}");
    }
    assert!(changes >= 4, "the target changes cell {changes} times");
    let g = s.entities.get(&guard).expect("the guard");
    assert_eq!(g.orders.len(), 1, "the Guard is kept");
}

/// A guard of a moving target plans again when the target's cell changes
/// and not between.
#[test]
fn sim_guard_plans_when_the_target_changes_cell() {
    count_plans(false, (2.5, 30.5), (30.5, 30.5), 40.5);
}

/// The same, with the target flying over ground on the far side of a wall
/// the guard cannot cross: every plan fails, and still only a change of the
/// target's cell causes one.
#[test]
fn sim_guard_of_an_unreachable_moving_target_plans_per_cell() {
    count_plans(true, (2.5, 30.5), (40.5, 30.5), 50.5);
}

/// A guard of a target that never moves and cannot be reached plans once
/// over 100 ticks, holding its order with an empty route and speed 0.
#[test]
fn sim_guard_of_an_unreachable_target_plans_once() {
    let w = world(
        vec![
            mobile("walker", MotionType::Land, 3.0),
            kind("post", &["STRUCTURE"]),
        ],
        Vec::new(),
        grid(true),
    );
    let mut s: State = state(&w, Q::ZERO, Q::ZERO);
    let target: EntityId = s.spawn(Army(1), "post", pos(50.5, 0.0, 30.5), q(100));
    let guard = s.spawn(Army(1), "walker", pos(5.5, 0.0, 30.5), q(100));
    step(
        &mut s,
        &[order_cmd(
            0,
            0,
            guard,
            Order::Guard { target },
            QueueMode::Replace,
        )],
    );
    for _ in 0..99 {
        step(&mut s, &[]);
    }
    assert_eq!(s.plans, 1, "one search over 100 ticks");
    let g = s.entities.get(&guard).expect("the guard");
    assert_eq!(g.orders.len(), 1, "the Guard is kept");
    assert_eq!(g.route.len(), 0, "the route is empty");
    assert_eq!(g.speed.to_bits(), 0.0_f64.to_bits());
    assert_eq!(s.counters.commands_ignored, 0);
    assert_eq!(
        (g.pos.x.to_bits(), g.pos.z.to_bits()),
        (5.5_f64.to_bits(), 30.5_f64.to_bits()),
        "the guard has not moved"
    );
}
