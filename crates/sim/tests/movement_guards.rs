//! Guards on the rules of the movement phase that no other test pins
//! (docs/specs/sim.md, SIM-064, SIM-065, SIM-068): unfinished entities are
//! skipped, a step into an impassable cell keeps its place with speed 0, a
//! Stop and a build head end a stride, a dropped Move ends a stride, and a
//! guard of a target on a cell it cannot enter heads for the first enterable
//! neighbour by index. All synthetic.

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
    Army, EntityId, Layer, MapGrid, MotionType, Order, Q, QueueMode, State, UnitKind,
};
use support::{build_at, builder, kind, order_cmd, pos, q, state, step, structure, world};

type Cell = (u32, u32);

fn mobile(id: &str, motion: MotionType, speed: f64) -> UnitKind {
    let mut k = kind(id, &["MOBILE"]);
    k.motion = motion;
    k.max_speed = Some(speed);
    k
}

/// A 32 by 32 land grid, every cell passable except `blocked`; air
/// everywhere.
fn grid(blocked: &[(u32, u32)]) -> MapGrid {
    let size = 32_u32;
    let cells = (size * size) as usize;
    let mut land = vec![true; cells];
    for (x, z) in blocked {
        land[(z * size + x) as usize] = false;
    }
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

fn move_to(x: f64, z: f64) -> Order {
    Order::Move { to: pos(x, 0.0, z) }
}

fn ent(s: &State, id: EntityId) -> &supercom_sim::Entity {
    s.entities.get(&id).expect("the entity exists")
}

/// SIM-064: the movement phase skips an unfinished entity whole, its queue
/// untouched, and moves it from the tick it completes.
#[test]
fn sim_unfinished_entities_are_skipped_by_the_movement_phase() {
    let mut rover = mobile("rover", MotionType::Land, 3.0);
    rover.build_time = Some(q(10));
    let w = world(vec![rover], Vec::new(), grid(&[]));
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let id = s
        .spawn_under_construction(Army(1), "rover", pos(4.5, 0.0, 4.5))
        .expect("an unfinished rover");
    step(
        &mut s,
        &[order_cmd(0, 0, id, move_to(12.5, 4.5), QueueMode::Replace)],
    );
    for _ in 0..5 {
        step(&mut s, &[]);
    }
    let e = ent(&s, id);
    assert_eq!(
        (e.pos.x.to_bits(), e.pos.z.to_bits()),
        (4.5_f64.to_bits(), 4.5_f64.to_bits())
    );
    assert_eq!(e.orders.len(), 1, "the queue is untouched");
    assert_eq!(e.goal, None);
    assert_eq!(e.speed.to_bits(), 0.0_f64.to_bits());
    s.entities.get_mut(&id).expect("exists").complete = true;
    step(&mut s, &[]);
    let e = ent(&s, id);
    assert_eq!(e.pos.x.to_bits(), (4.5_f64 + 3.0 * 0.1).to_bits());
    assert_eq!(e.speed.to_bits(), 3.0_f64.to_bits());
}

/// SIM-065: a step that would enter an impassable cell keeps the entity's
/// place and route and sets its speed to 0, tick after tick, never clipping
/// into the cell.
#[test]
fn sim_a_step_into_a_cliff_keeps_place_and_route_at_speed_zero() {
    let w = world(
        vec![mobile("walker", MotionType::Land, 3.0)],
        Vec::new(),
        grid(&[(6, 4)]),
    );
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let id = s.spawn(Army(1), "walker", pos(4.5, 0.0, 4.5), q(100));
    // A route that leads into the wall cell, as a stale one could.
    let goal = pos(8.5, 0.0, 4.5);
    {
        let e = s.entities.get_mut(&id).expect("exists");
        e.orders.push_back(Order::Move { to: goal });
        e.goal = Some(goal);
        e.route = vec![(6, 4)];
    }
    let mut stuck_at = None;
    for tick in 0..20 {
        let before = ent(&s, id).pos;
        step(&mut s, &[]);
        let e = ent(&s, id);
        assert!(
            e.pos.x < 6.0,
            "tick {tick}: x {} is inside the wall",
            e.pos.x
        );
        if e.speed.to_bits() == 0.0_f64.to_bits() {
            stuck_at = Some(tick);
            assert_eq!(e.pos, before, "the blocked step keeps its place");
            break;
        }
        assert_eq!(e.speed.to_bits(), 3.0_f64.to_bits(), "tick {tick}");
    }
    let tick = stuck_at.expect("the unit is blocked by the wall");
    assert!(tick > 3, "it walked first, blocked at tick {tick}");
    let held = ent(&s, id).pos;
    for _ in 0..5 {
        step(&mut s, &[]);
        let e = ent(&s, id);
        assert_eq!(e.pos, held);
        assert_eq!(e.speed.to_bits(), 0.0_f64.to_bits());
        assert_eq!(e.route, vec![(6, 4)], "the route is kept");
        assert_eq!(e.goal, Some(goal));
        assert_eq!(e.orders.len(), 1);
    }
}

fn engineer(speed: f64) -> UnitKind {
    let mut k = builder("eng", 10, &["gen"]);
    k.motion = MotionType::Land;
    k.max_speed = Some(speed);
    k.max_acceleration = Some(1.0);
    k
}

/// A world with a moving engineer 15 ticks into a Move.
fn moving_engineer() -> (State, EntityId) {
    let w = world(
        vec![engineer(5.0), structure("gen", 60, 600, 100)],
        Vec::new(),
        grid(&[(20, 20)]),
    );
    let mut s = state(&w, q(1000), q(100_000));
    let id = s.spawn(Army(1), "eng", pos(5.5, 0.0, 5.5), q(100));
    step(
        &mut s,
        &[order_cmd(0, 0, id, move_to(30.5, 5.5), QueueMode::Replace)],
    );
    for _ in 0..14 {
        step(&mut s, &[]);
    }
    assert!(ent(&s, id).speed > 1.0, "the engineer is under way");
    assert!(ent(&s, id).route.len() > 1);
    (s, id)
}

/// SIM-068: a builder given a Stop in Replace mode with a Move appended in
/// the same tick starts that Move from speed 0 with a fresh route, because
/// the Stop is run by the one helper that clears route, goal, and speed.
#[test]
fn sim_a_stop_before_a_move_starts_the_move_from_rest() {
    let (mut s, id) = moving_engineer();
    let tick = s.tick.0;
    step(
        &mut s,
        &[
            order_cmd(tick, 0, id, Order::Stop, QueueMode::Replace),
            order_cmd(tick, 1, id, move_to(5.5, 25.5), QueueMode::Append),
        ],
    );
    let e = ent(&s, id);
    assert_eq!(
        e.speed.to_bits(),
        (0.0_f64 + 1.0 * 0.1).to_bits(),
        "one acceleration step from rest"
    );
    assert_eq!(e.goal, Some(pos(5.5, 0.0, 25.5)));
    assert_eq!(e.orders.len(), 1);
}

/// SIM-068: an entity whose head is neither a Move nor a Guard has its route
/// and goal cleared and its speed set to 0 by the movement phase.
#[test]
fn sim_a_build_head_stops_a_moving_unit() {
    let (mut s, id) = moving_engineer();
    let tick = s.tick.0;
    // Far out of range of the site, so the order waits at the head.
    step(
        &mut s,
        &[order_cmd(
            tick,
            0,
            id,
            build_at("gen", 25.5, 28.5),
            QueueMode::Replace,
        )],
    );
    let e = ent(&s, id);
    assert_eq!(e.orders.len(), 1, "the build order waits at the head");
    assert_eq!(e.speed.to_bits(), 0.0_f64.to_bits());
    assert_eq!(e.route.len(), 0);
    assert_eq!(e.goal, None);
}

/// SIM-068: an entity left with no order has speed 0, route and goal
/// cleared, whether its last Move completed or was dropped mid-stride.
#[test]
fn sim_an_empty_queue_leaves_a_unit_at_rest() {
    let (mut s, id) = moving_engineer();
    let tick = s.tick.0;
    // A Move onto a wall cell is dropped and counted; the old Move was
    // replaced, so the queue is empty with the unit mid-stride.
    step(
        &mut s,
        &[order_cmd(
            tick,
            0,
            id,
            move_to(20.5, 20.5),
            QueueMode::Replace,
        )],
    );
    let e = ent(&s, id);
    assert_eq!(s.counters.commands_ignored, 1);
    assert_eq!(e.orders.len(), 0);
    assert_eq!(e.speed.to_bits(), 0.0_f64.to_bits());
    assert_eq!(e.route.len(), 0);
    assert_eq!(e.goal, None);
}

/// SIM-068: a guard of a target standing on a cell it cannot enter plans to
/// the first enterable cell of the target's eight neighbours in ascending
/// index, keeping the target's own cell as its goal.
#[test]
fn sim_a_guard_heads_for_the_first_enterable_neighbour_by_index() {
    // Neighbour indices ascend (9,9), (10,9), (11,9), (9,10), (11,10), ...
    let cases: [(&[Cell], Cell); 3] = [
        (&[(10, 10)], (9, 9)),
        (&[(10, 10), (9, 9), (10, 9)], (11, 9)),
        (&[(10, 10), (9, 9), (10, 9), (11, 9), (9, 10)], (11, 10)),
    ];
    for (blocked, want) in cases {
        let w = world(
            vec![
                mobile("walker", MotionType::Land, 1.0),
                mobile("flyer", MotionType::Air, 1.0),
            ],
            Vec::new(),
            grid(blocked),
        );
        let mut s = state(&w, Q::ZERO, Q::ZERO);
        let target = s.spawn(Army(1), "flyer", pos(10.5, 0.0, 10.5), q(100));
        let guard = s.spawn(Army(1), "walker", pos(2.5, 0.0, 2.5), q(100));
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
        let e = ent(&s, guard);
        assert_eq!(e.route.last(), Some(&want), "blocked {blocked:?}");
        assert_eq!(e.goal, Some(pos(10.5, 0.0, 10.5)), "the target's own cell");
    }
}
