//! The movement phase (docs/specs/sim.md, SIM-064 to SIM-070): layer and
//! height assignment, the integration rule, route following, orders, guard
//! following, per-tick time, and the water layers. All synthetic: no
//! install, no file. Each expected value is computed here, from the
//! formulas the clauses state and the `libm` functions they name, never by
//! calling the code under test.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers, and each scenario reads top to bottom"
)]

mod support;

use std::collections::BTreeMap;
use std::f64::consts::PI;

use supercom_sim::{
    ARRIVAL_RADIUS, Army, EntityId, Error, FOLLOW_RANGE, IngestError, Layer, MapGrid, MotionType,
    Order, Pos, Q, QueueMode, State, TICK_SECONDS, UnitKind, cell_centre, cell_of, plan_route,
};
use support::{builder, flat_map, kind, order_cmd, pos, q, state, step, structure, world};

// ---- fixtures ---------------------------------------------------------------

/// A mobile kind of `motion` with `speed` as its maximum, and none of the
/// optional dynamics fields.
fn mobile(id: &str, motion: MotionType, speed: f64) -> UnitKind {
    let mut k = kind(id, &["MOBILE"]);
    k.motion = motion;
    k.max_speed = Some(speed);
    k
}

/// A `width` by `height` grid, flat, with every cell land-passable except
/// `blocked`, and air everywhere.
fn open_grid(width: u32, height: u32, blocked: &[(u32, u32)]) -> MapGrid {
    let cells = (width * height) as usize;
    let mut land = vec![true; cells];
    for (x, z) in blocked {
        land[(z * width + x) as usize] = false;
    }
    let mut passable = BTreeMap::new();
    passable.insert(Layer::Land, land);
    passable.insert(Layer::Air, vec![true; cells]);
    MapGrid {
        width,
        height,
        heights: vec![Q::ZERO; ((width + 1) * (height + 1)) as usize],
        passable,
        water: None,
    }
}

fn move_to(x: f64, z: f64) -> Order {
    Order::Move { to: pos(x, 0.0, z) }
}

/// The floor of a coordinate as a cell index, by the test's own conversion.
fn floor_cell(v: f64) -> u32 {
    assert!(
        (0.0..1.0e6).contains(&v),
        "a coordinate inside the test maps"
    );
    format!("{:.0}", v.floor())
        .parse()
        .expect("a small integer")
}

fn ent(s: &State, id: EntityId) -> &supercom_sim::Entity {
    s.entities.get(&id).expect("the entity exists")
}

fn centre(cell: (u32, u32)) -> (f64, f64) {
    (f64::from(cell.0) + 0.5, f64::from(cell.1) + 0.5)
}

fn wrap(angle: f64) -> f64 {
    let mut a = angle;
    while a > PI {
        a -= 2.0 * PI;
    }
    while a <= -PI {
        a += 2.0 * PI;
    }
    a
}

// ---- the integration rule, restated (SIM-065) -------------------------------

/// The dynamics a kind states; `None` is an absent field.
struct Dynamics {
    max_speed: f64,
    acceleration: Option<f64>,
    brake: Option<f64>,
    turn_rate: Option<f64>,
}

/// An entity as the rule sees it, on an open grid.
struct Body {
    x: f64,
    z: f64,
    heading: f64,
    speed: f64,
    route: Vec<(u32, u32)>,
    goal: (f64, f64),
    braked: u32,
}

/// One tick of SIM-065's rule, in the order of operations the clause states.
/// Returns true when the move completes.
fn reference_step(b: &mut Body, d: &Dynamics) -> bool {
    let dt = 0.1_f64;
    let (tx, tz) = b.route.first().map_or(b.goal, |c| centre(*c));
    let dx = tx - b.x;
    let dz = tz - b.z;
    let desired = libm::atan2(dz, dx);
    let delta = wrap(desired - b.heading);
    let step = match d.turn_rate {
        Some(rate) => {
            let max_turn = rate * (PI / 180.0) * dt;
            delta.max(-max_turn).min(max_turn)
        }
        None => delta,
    };
    let heading = wrap(b.heading + step);

    let mut points = vec![(b.x, b.z)];
    points.extend(b.route.iter().map(|c| centre(*c)));
    points.push(b.goal);
    let mut remaining = 0.0_f64;
    for w in points.windows(2) {
        let ddx = w[1].0 - w[0].0;
        let ddz = w[1].1 - w[0].1;
        remaining += libm::sqrt(ddx * ddx + ddz * ddz);
    }
    let speed = match d.brake {
        Some(brake) if remaining <= b.speed * b.speed / (2.0 * brake) => {
            b.braked += 1;
            (b.speed - brake * dt).max(0.0)
        }
        _ => match d.acceleration {
            Some(a) => d.max_speed.min(b.speed + a * dt),
            None => d.max_speed,
        },
    };
    let s = speed * dt;
    let distance = libm::sqrt(dx * dx + dz * dz);
    let (nx, nz) = if b.route.is_empty() && s >= distance {
        b.goal
    } else {
        (b.x + s * libm::cos(heading), b.z + s * libm::sin(heading))
    };
    b.x = nx;
    b.z = nz;
    b.heading = heading;
    b.speed = speed;
    while let Some(c) = b.route.first() {
        let (cx, cz) = centre(*c);
        let (ddx, ddz) = (cx - nx, cz - nz);
        if ddx * ddx + ddz * ddz <= ARRIVAL_RADIUS * ARRIVAL_RADIUS {
            b.route.remove(0);
        } else {
            break;
        }
    }
    let (gx, gz) = (b.goal.0 - nx, b.goal.1 - nz);
    if gx * gx + gz * gz <= ARRIVAL_RADIUS * ARRIVAL_RADIUS {
        b.speed = 0.0;
        b.route.clear();
        return true;
    }
    false
}

/// A straight route of `n` cells from `from` along `(dx, dz)` unit steps.
fn straight(from: (u32, u32), step: (i32, i32), n: u32) -> Vec<(u32, u32)> {
    (1..=n)
        .map(|i| {
            let k = i32::try_from(i).expect("a small count");
            (
                u32::try_from(i32::try_from(from.0).expect("small") + step.0 * k).expect("in map"),
                u32::try_from(i32::try_from(from.1).expect("small") + step.1 * k).expect("in map"),
            )
        })
        .collect()
}

// ---- SIM-064 ----------------------------------------------------------------

/// SIM-064: the cell is one world unit; a mobile entity's layer and height
/// follow its kind's motion type and the terrain; an unknown or immobile
/// kind is skipped whole.
#[test]
fn sim_layer_assignment_from_motion_and_terrain() {
    // The cell helpers: floor of each coordinate, centre at a half.
    assert_eq!(cell_of(&pos(2.7, 9.0, 3.2)), Some((2, 3)));
    assert_eq!(cell_of(&pos(0.0, 0.0, 0.999)), Some((0, 0)));
    assert_eq!(cell_of(&pos(-0.001, 0.0, 1.0)), None);
    assert_eq!(cell_of(&pos(1.0, 0.0, -2.0)), None);
    assert_eq!(cell_of(&pos(f64::NAN, 0.0, 1.0)), None);
    assert_eq!(cell_of(&pos(4_294_967_296.0, 0.0, 1.0)), None);
    assert_eq!(
        cell_of(&pos(4_294_967_295.5, 0.0, 1.0)),
        Some((u32::MAX, 1))
    );
    assert_eq!(cell_centre((2, 3)), (2.5, 3.5));

    // An 8x8 ramp rising half a unit per column: sample (x, z) is 64 raw at
    // the scale 2^-7, so height_at(x, _) is x / 2 and every cell's slope is
    // 0.5, under the 0.75 of SIM-047.
    let samples: Vec<i16> = (0..81)
        .map(|i| i16::try_from(64 * (i % 9)).expect("a small sample"))
        .collect();
    let map = MapGrid::from_heightmap(8, 8, &samples, 0.007_812_5).expect("a ramp");
    let mut plane = mobile("plane", MotionType::Air, 3.0);
    plane.elevation = Some(5.0);
    let mut hover = mobile("hover", MotionType::Hover, 2.0);
    hover.elevation = Some(0.5);
    let w = world(
        vec![
            mobile("tank", MotionType::Land, 2.0),
            plane,
            hover,
            mobile("boat", MotionType::Water, 2.0),
            kind("wall", &["STRUCTURE"]),
        ],
        Vec::new(),
        map,
    );
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let at = pos(2.5, 99.0, 3.5);
    let ids: Vec<EntityId> = ["tank", "plane", "hover", "boat", "wall", "ghost"]
        .iter()
        .map(|b| s.spawn(Army(1), b, at, q(100)))
        .collect();
    let commands: Vec<_> = ids
        .iter()
        .zip(0_u32..)
        .map(|(id, seq)| order_cmd(0, seq, *id, move_to(6.5, 3.5), QueueMode::Replace))
        .collect();
    step(&mut s, &commands);
    let [tank, plane, hover, boat, wall, ghost] = ids[..] else {
        panic!("six ids");
    };

    // Layer by motion type, whether or not the grid exists (SIM-064).
    assert_eq!(ent(&s, tank).layer, Layer::Land);
    assert_eq!(ent(&s, plane).layer, Layer::Air);
    assert_eq!(ent(&s, hover).layer, Layer::Land);
    assert_eq!(ent(&s, boat).layer, Layer::Water);
    // Height: the ground under a land unit, plus the elevation of an air or
    // hover unit; column 2 stands at 1.0.
    assert_eq!(ent(&s, tank).pos.y.to_bits(), 1.0_f64.to_bits());
    assert_eq!(ent(&s, plane).pos.y.to_bits(), 6.0_f64.to_bits());
    assert_eq!(ent(&s, hover).pos.y.to_bits(), 1.5_f64.to_bits());
    // The naval Move finds no route without a water grid: dropped, counted.
    assert!(ent(&s, boat).orders.is_empty());
    assert_eq!(s.counters.commands_ignored, 1);
    assert_eq!(s.counters.commands_applied, 6);
    // An immobile kind and an unknown blueprint are skipped whole: the
    // order stays queued and nothing of the entity changes.
    for id in [wall, ghost] {
        assert_eq!(ent(&s, id).orders.front(), Some(&move_to(6.5, 3.5)));
        assert_eq!(ent(&s, id).layer, Layer::Land);
        assert_eq!(ent(&s, id).pos, at);
        assert_eq!(ent(&s, id).route, Vec::new());
    }

    // Height follows the cell the entity is in after every move.
    for _ in 0..12 {
        step(&mut s, &[]);
        let ground = 0.5 * f64::from(floor_cell(ent(&s, tank).pos.x));
        assert_eq!(ent(&s, tank).pos.y.to_bits(), ground.to_bits());
        let ground = 0.5 * f64::from(floor_cell(ent(&s, plane).pos.x));
        assert_eq!(ent(&s, plane).pos.y.to_bits(), (ground + 5.0).to_bits());
    }
    assert!(ent(&s, tank).pos.x > 2.5, "the tank has moved");
}

// ---- SIM-065 ----------------------------------------------------------------

/// Run `d` against the engine on an open grid from `start` along `route` to
/// `goal`, comparing every tick's bits with the restated rule; returns the
/// final reference body and the tick at which the move completed, if any.
fn compare_ticks(
    kind: UnitKind,
    d: &Dynamics,
    start: (f64, f64),
    goal: (f64, f64),
    route: Vec<(u32, u32)>,
    ticks: u32,
) -> (Body, Option<u32>, Vec<f64>, Vec<f64>) {
    let id = kind.id.clone();
    let w = world(vec![kind], Vec::new(), open_grid(32, 32, &[]));
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let e = s.spawn(Army(1), &id, pos(start.0, 0.0, start.1), q(100));
    let mut body = Body {
        x: start.0,
        z: start.1,
        heading: 0.0,
        speed: 0.0,
        route,
        goal,
        braked: 0,
    };
    let mut speeds = Vec::new();
    let mut headings = Vec::new();
    let mut done = None;
    for tick in 0..ticks {
        let cmds = if tick == 0 {
            vec![order_cmd(
                0,
                0,
                e,
                move_to(goal.0, goal.1),
                QueueMode::Replace,
            )]
        } else {
            Vec::new()
        };
        step(&mut s, &cmds);
        let completed = reference_step(&mut body, d);
        let got = s.entities.get(&e).expect("the entity exists");
        assert_eq!(got.pos.x.to_bits(), body.x.to_bits(), "x at tick {tick}");
        assert_eq!(got.pos.z.to_bits(), body.z.to_bits(), "z at tick {tick}");
        assert_eq!(
            got.speed.to_bits(),
            body.speed.to_bits(),
            "speed at tick {tick}"
        );
        assert_eq!(
            got.heading.to_bits(),
            body.heading.to_bits(),
            "heading at tick {tick}"
        );
        assert_eq!(got.route, body.route, "route at tick {tick}");
        speeds.push(body.speed);
        headings.push(body.heading);
        if completed {
            assert_eq!(s.counters.moves_completed, 1);
            assert!(got.orders.is_empty() && got.goal.is_none() && got.route.is_empty());
            done = Some(tick);
            break;
        }
        assert_eq!(
            got.goal,
            Some(pos(goal.0, 0.0, goal.1)),
            "goal at tick {tick}"
        );
    }
    (body, done, speeds, headings)
}

/// SIM-065: speed accelerates by the kind's acceleration, brakes inside the
/// braking distance, and heading turns at the kind's turn rate, in the
/// clause's order of f64 operations; absent fields take the instant
/// defaults.
#[test]
fn sim_integration_accelerates_brakes_and_turns() {
    // A runner: speed 2, acceleration 1, brake 2, turn rate 90, a goal 10
    // units along +x.
    let mut runner = mobile("runner", MotionType::Land, 2.0);
    runner.max_acceleration = Some(1.0);
    runner.max_brake = Some(2.0);
    runner.turn_rate = Some(90.0);
    let d = Dynamics {
        max_speed: 2.0,
        acceleration: Some(1.0),
        brake: Some(2.0),
        turn_rate: Some(90.0),
    };
    let (body, done, speeds, _headings) = compare_ticks(
        runner,
        &d,
        (2.5, 5.5),
        (12.5, 5.5),
        straight((2, 5), (1, 0), 10),
        400,
    );
    // The first three speeds in the formula's order: min(2, 0 + 1 * 0.1),
    // then that sum + 0.1, then that + 0.1, which is not 0.3 in binary64.
    let s1 = 0.0_f64 + 1.0 * 0.1;
    let s2 = s1 + 1.0 * 0.1;
    let s3 = s2 + 1.0 * 0.1;
    assert_ne!(s3.to_bits(), 0.3_f64.to_bits());
    for (got, want) in speeds.iter().zip([s1, s2, s3]) {
        assert_eq!(got.to_bits(), want.to_bits());
    }
    assert_eq!(
        body.heading.to_bits(),
        0.0_f64.to_bits(),
        "no turning on a straight line"
    );
    assert!(
        speeds.iter().any(|v| v.to_bits() == 2.0_f64.to_bits()),
        "the speed reaches the maximum"
    );
    assert!(body.braked > 0, "the braking distance is reached");
    assert!(
        speeds.windows(2).any(|w| w[1] < w[0]),
        "the speed falls while braking"
    );
    let tick = done.expect("the move completes");
    assert!(tick > 20 && tick < 399, "completion tick {tick}");
    let gx = body.x - 12.5;
    assert!(
        gx * gx <= ARRIVAL_RADIUS * ARRIVAL_RADIUS,
        "stops within the arrival radius"
    );

    // A kind with no acceleration, brake, or turn rate reaches its maximum
    // at once and turns at once: heading is atan2(1, 0) after one tick.
    let snappy = mobile("snappy", MotionType::Land, 3.0);
    let d = Dynamics {
        max_speed: 3.0,
        acceleration: None,
        brake: None,
        turn_rate: None,
    };
    let (body, done, speeds, headings) = compare_ticks(
        snappy,
        &d,
        (2.5, 2.5),
        (2.5, 12.5),
        straight((2, 2), (0, 1), 10),
        400,
    );
    assert_eq!(speeds[0].to_bits(), 3.0_f64.to_bits());
    assert!(done.is_some(), "the move completes");
    assert_eq!(body.braked, 0);
    assert_eq!(headings[0].to_bits(), libm::atan2(1.0, 0.0).to_bits());

    // A turn-limited kind turns by turn_rate * (PI / 180) * 0.1 a tick.
    let mut turner = mobile("turner", MotionType::Land, 2.0);
    turner.turn_rate = Some(90.0);
    let d = Dynamics {
        max_speed: 2.0,
        acceleration: None,
        brake: None,
        turn_rate: Some(90.0),
    };
    let (body, _, _, headings) = compare_ticks(
        turner,
        &d,
        (2.5, 2.5),
        (2.5, 12.5),
        straight((2, 2), (0, 1), 10),
        12,
    );
    let max_turn = 90.0_f64 * (PI / 180.0) * 0.1;
    assert_eq!(headings[0].to_bits(), max_turn.to_bits());
    assert!(
        body.heading > max_turn * 2.0,
        "the heading has turned several steps"
    );
}

// ---- SIM-068 ----------------------------------------------------------------

/// SIM-068: a Move plans a route once, follows it cell by cell, arrives,
/// pops the order, and counts it; an impassable goal is dropped and counted;
/// a Stop halts the unit with speed 0 and the route cleared.
#[test]
fn sim_move_follows_route_and_completes() {
    // A wall at x = 5 across rows 0 to 9 of a 12 by 12 map leaves a gap in
    // rows 10 and 11.
    let wall: Vec<(u32, u32)> = (0..10).map(|z| (5, z)).collect();
    let grid = open_grid(12, 12, &wall);
    let w = world(
        vec![mobile("walker", MotionType::Land, 4.0)],
        Vec::new(),
        grid.clone(),
    );
    let planned = plan_route(&grid, Layer::Land, (1, 5), (9, 5));
    let planned = planned.unwrap_or_default();
    assert!(!planned.is_empty(), "a route exists around the wall");
    assert_eq!(planned.last(), Some(&(9, 5)));
    assert!(
        planned.iter().any(|c| c.0 == 5 && c.1 >= 10),
        "the route uses the gap"
    );

    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let e = s.spawn(Army(1), "walker", pos(1.5, 0.0, 5.5), q(100));
    step(
        &mut s,
        &[order_cmd(0, 0, e, move_to(9.5, 5.5), QueueMode::Replace)],
    );
    let mut shortest = usize::MAX;
    let mut done = false;
    for tick in 0..400 {
        let got = s.entities.get(&e).expect("exists");
        // The route still to visit is always a suffix of the planned one,
        // so cells are consumed from the front in order.
        assert!(got.route.len() <= planned.len(), "tick {tick}");
        assert_eq!(
            got.route[..],
            planned[planned.len() - got.route.len()..],
            "tick {tick}"
        );
        assert!(got.route.len() <= shortest, "cells are never un-consumed");
        shortest = got.route.len();
        let cell = (floor_cell(got.pos.x), floor_cell(got.pos.z));
        assert!(
            !wall.contains(&cell),
            "tick {tick}: inside the wall at {cell:?}"
        );
        if s.counters.moves_completed == 1 {
            done = true;
            break;
        }
        step(&mut s, &[]);
    }
    assert!(done, "the move completes");
    let got = s.entities.get(&e).expect("exists");
    let (dx, dz) = (got.pos.x - 9.5, got.pos.z - 5.5);
    assert!(dx * dx + dz * dz <= ARRIVAL_RADIUS * ARRIVAL_RADIUS);
    assert!(got.orders.is_empty() && got.route.is_empty() && got.goal.is_none());
    assert_eq!(got.speed.to_bits(), 0.0_f64.to_bits());
    assert_eq!(s.counters.commands_ignored, 0);

    // A Move to an impassable goal is dropped and counted.
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let e = s.spawn(Army(1), "walker", pos(1.5, 0.0, 5.5), q(100));
    step(
        &mut s,
        &[order_cmd(0, 0, e, move_to(5.5, 3.5), QueueMode::Replace)],
    );
    assert!(s.entities.get(&e).expect("exists").orders.is_empty());
    assert_eq!(s.counters.commands_ignored, 1);
    assert_eq!(s.counters.moves_completed, 0);
    // So is a Move off the map.
    step(
        &mut s,
        &[order_cmd(1, 1, e, move_to(-3.0, 3.5), QueueMode::Replace)],
    );
    assert_eq!(s.counters.commands_ignored, 2);

    // A Stop mid-route halts the unit at once.
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let e = s.spawn(Army(1), "walker", pos(1.5, 0.0, 5.5), q(100));
    step(
        &mut s,
        &[order_cmd(0, 0, e, move_to(9.5, 5.5), QueueMode::Replace)],
    );
    for _ in 0..4 {
        step(&mut s, &[]);
    }
    let moving = s.entities.get(&e).expect("exists").clone();
    assert!(moving.speed > 0.0 && !moving.route.is_empty() && moving.goal.is_some());
    step(
        &mut s,
        &[order_cmd(5, 1, e, Order::Stop, QueueMode::Replace)],
    );
    let stopped = s.entities.get(&e).expect("exists").clone();
    assert_eq!(stopped.speed.to_bits(), 0.0_f64.to_bits());
    assert!(stopped.route.is_empty() && stopped.goal.is_none() && stopped.orders.is_empty());
    assert_eq!(
        stopped.pos, moving.pos,
        "the stopping tick does not move it"
    );
    step(&mut s, &[]);
    assert_eq!(s.entities.get(&e).expect("exists").pos, moving.pos);
    assert_eq!(s.counters.moves_completed, 0);
}

// ---- SIM-068: guard following -----------------------------------------------

fn dist2(a: &Pos, b: &Pos) -> f64 {
    (a.x - b.x) * (a.x - b.x) + (a.z - b.z) * (a.z - b.z)
}

/// SIM-068: a guard farther than `FOLLOW_RANGE` from its target plans a route
/// to it and follows, stops within the range keeping its Guard, assists
/// from there by the build rule, and a Guard of a target that is gone is
/// dropped and counted.
#[test]
fn sim_guard_follows_target_into_range() {
    let mut helper = builder("helper", 10, &["gen"]);
    helper.motion = MotionType::Land;
    helper.max_speed = Some(5.0);
    let w = world(
        vec![
            builder("boss", 10, &["gen"]),
            helper,
            structure("gen", 60, 600, 100_000),
        ],
        Vec::new(),
        flat_map(40),
    );
    let mut s = state(&w, q(1000), q(1_000_000));
    let boss = s.spawn(Army(1), "boss", pos(10.5, 0.0, 10.5), q(100));
    let helper = s.spawn(Army(1), "helper", pos(30.5, 0.0, 10.5), q(100));
    step(
        &mut s,
        &[
            order_cmd(
                0,
                0,
                boss,
                Order::BuildMobile {
                    blueprint: "gen".to_owned(),
                    at: pos(11.5, 0.0, 10.5),
                },
                QueueMode::Replace,
            ),
            order_cmd(
                0,
                1,
                helper,
                Order::Guard { target: boss },
                QueueMode::Replace,
            ),
        ],
    );
    let site = s
        .entities
        .get(&boss)
        .expect("exists")
        .work
        .expect("the boss is building");
    let range2 = FOLLOW_RANGE * FOLLOW_RANGE;
    let mut last = dist2(
        &s.entities.get(&helper).expect("e").pos,
        &s.entities.get(&boss).expect("e").pos,
    );
    let mut stopped_at = None;
    for tick in 0..120 {
        step(&mut s, &[]);
        let (h, b) = (
            s.entities.get(&helper).expect("exists").clone(),
            s.entities.get(&boss).expect("exists").clone(),
        );
        let now = dist2(&h.pos, &b.pos);
        if last > range2 {
            assert!(now < last, "tick {tick}: still approaching");
        }
        if last <= range2 {
            // The phase that began within range stops the guard.
            assert_eq!(h.speed.to_bits(), 0.0_f64.to_bits(), "tick {tick}");
            assert!(h.route.is_empty() && h.goal.is_none());
            assert_eq!(now.to_bits(), last.to_bits(), "tick {tick}: it stays put");
            assert_eq!(h.orders.front(), Some(&Order::Guard { target: boss }));
            stopped_at = Some(tick);
            break;
        }
        last = now;
    }
    assert!(stopped_at.is_some(), "the guard comes into range and stops");
    assert!(last <= range2 && last > 0.0);

    // From range it contributes to the boss's work by the build rule: the
    // site gains two builders' rates (1 each per tick) a tick.
    step(&mut s, &[]);
    assert_eq!(s.entities.get(&helper).expect("exists").work, Some(site));
    let before = s.entities.get(&site).expect("site").built;
    step(&mut s, &[]);
    let after = s.entities.get(&site).expect("site").built;
    assert_eq!(after.saturating_sub(before), q(2));

    // A Guard of a target that is gone is dropped and counted.
    let ignored = s.counters.commands_ignored;
    s.entities.remove(&boss);
    step(&mut s, &[]);
    assert!(s.entities.get(&helper).expect("exists").orders.is_empty());
    assert_eq!(s.counters.commands_ignored, ignored + 1);
}

// ---- SIM-069 ----------------------------------------------------------------

/// SIM-069: a tick is a tenth of a second, so a unit at maximum speed 3
/// advances 0.3 per tick along +x.
#[test]
fn sim_speeds_apply_per_tick_as_a_tenth() {
    assert_eq!(TICK_SECONDS.to_bits(), 0.1_f64.to_bits());
    let w = world(
        vec![mobile("cart", MotionType::Land, 3.0)],
        Vec::new(),
        open_grid(32, 32, &[]),
    );
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let e = s.spawn(Army(1), "cart", pos(2.5, 0.0, 5.5), q(100));
    step(
        &mut s,
        &[order_cmd(0, 0, e, move_to(22.5, 5.5), QueueMode::Replace)],
    );
    // Each tick advances by the speed times the tenth, exactly as written.
    let advance = 3.0_f64 * 0.1;
    let mut x = 2.5_f64;
    for tick in 0..5 {
        x += advance * libm::cos(0.0);
        let got = s.entities.get(&e).expect("exists").pos;
        assert_eq!(got.x.to_bits(), x.to_bits(), "tick {tick}");
        assert_eq!(got.z.to_bits(), 5.5_f64.to_bits(), "tick {tick}");
        assert!((advance - 0.3).abs() < 1e-12);
        step(&mut s, &[]);
    }
    assert!(
        (x - 4.0).abs() < 1e-9,
        "five ticks cover one and a half units"
    );
}

// ---- SIM-070 ----------------------------------------------------------------

/// Raw samples (2^-7 units) of the strip map: five zones of three sample
/// columns each, from dry (height 5.0) to 30 under the surface 4.0.
const ZONE_HEIGHTS: [i16; 5] = [640, 508, 448, 320, -3328];

fn strip_samples() -> Vec<i16> {
    let mut samples = Vec::new();
    for _row in 0..5 {
        for col in 0..15_usize {
            samples.push(ZONE_HEIGHTS[col / 3]);
        }
    }
    samples
}

/// SIM-070: the surface converts once and exactly or is refused; a dry map
/// has no water grids; on a wet map each layer's grid follows the depth
/// thresholds of its footprint rows, compared cell by cell with the test's
/// own integer arithmetic; layers assign by motion type and the cell's
/// depth; water-layer units stand on the surface and subs below it.
#[test]
fn sim_water_layers_from_depth() {
    let samples = strip_samples();
    let scale = 0.007_812_5_f32;
    // 17.5 is exact, and so is 0.1 as an f32 (27 fractional bits); 2^-40
    // needs more fractional bits than the type has, 3e9 is beyond 2^31, and
    // NaN and infinity are not numbers.
    let wet =
        MapGrid::from_heightmap_with_water(14, 4, &samples, scale, Some(4.0)).expect("a wet strip");
    assert_eq!(wet.water, Some(q(4)));
    let exact = MapGrid::from_heightmap_with_water(14, 4, &samples, scale, Some(17.5))
        .expect("17.5 is exact");
    assert_eq!(exact.water, Some(Q::from_bits(35 << 31)));
    assert_eq!(
        MapGrid::from_heightmap_with_water(14, 4, &samples, scale, Some(0.1))
            .expect("0.1 is exact as an f32")
            .water,
        // 0.1_f32 is 13421773 * 2^-27, which is 13421773 << 5 in raw bits.
        Some(Q::from_bits(429_496_736)),
    );
    for bad in [
        f32::from_bits(0x2B80_0000),
        3.0e9_f32,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        assert_eq!(
            MapGrid::from_heightmap_with_water(14, 4, &samples, scale, Some(bad)),
            Err(Error::Ingest(IngestError::WaterSurface(bad)))
        );
    }
    assert!(matches!(
        MapGrid::from_heightmap_with_water(14, 4, &samples, scale, Some(f32::NAN)),
        Err(Error::Ingest(IngestError::WaterSurface(v))) if v.is_nan()
    ));
    // A dry map has no water, seabed, or sub grids, so they read false.
    let dry = MapGrid::from_heightmap_with_water(14, 4, &samples, scale, None).expect("dry");
    assert_eq!(dry.water, None);
    for layer in [Layer::Seabed, Layer::Water, Layer::Sub] {
        assert!(!dry.passable.contains_key(&layer));
        assert!(!dry.passable(layer, 12, 1));
    }
    assert_eq!(
        dry.passable,
        MapGrid::from_heightmap(14, 4, &samples, scale)
            .expect("dry")
            .passable
    );

    // A wet map has all three grids, even where no cell is submerged.
    for layer in [
        Layer::Land,
        Layer::Air,
        Layer::Seabed,
        Layer::Water,
        Layer::Sub,
    ] {
        assert!(wet.passable.contains_key(&layer), "{layer:?}");
    }
    // Every cell against the test's own computation, in raw 2^-7 units: the
    // depth is the surface (512) less the corner sample, a slope is the
    // largest edge difference, and the thresholds are 0.05 (6 raw, since a
    // depth is a whole number of raw units), 1.5 (192), 25 (3200), and 0.75
    // (96).
    let at = |x: usize, z: usize| i32::from(samples[z * 15 + x]);
    let mut counts = [0_usize; 4];
    for z in 0..4_usize {
        for x in 0..14_usize {
            let c = [at(x, z), at(x + 1, z), at(x + 1, z + 1), at(x, z + 1)];
            let slope = (0..4)
                .map(|i| (c[i] - c[(i + 1) % 4]).abs())
                .max()
                .expect("edges");
            let depth = 512 - c[0];
            let flat = slope <= 96;
            let want = [
                (Layer::Land, flat && depth <= 6),
                (Layer::Seabed, flat && depth > 0 && depth <= 3200),
                (Layer::Water, depth >= 192),
                (Layer::Sub, depth >= 192),
            ];
            let (cx, cz) = (u32::try_from(x).expect("x"), u32::try_from(z).expect("z"));
            for (i, (layer, ok)) in want.into_iter().enumerate() {
                assert_eq!(wet.passable(layer, cx, cz), ok, "{layer:?} at ({x}, {z})");
                counts[i] += usize::from(ok);
            }
            assert!(wet.passable(Layer::Air, cx, cz));
        }
    }
    // The strip exercises every threshold: each grid has both values.
    for n in counts {
        assert!(n > 0 && n < 56, "a grid with both values, {n}");
    }

    // Layers by motion type and depth, and heights on and under the surface.
    let mut amph = mobile("amph", MotionType::Amphibious, 2.0);
    amph.elevation = None;
    let mut sub = mobile("sub", MotionType::SurfacingSub, 2.0);
    sub.elevation = Some(-3.0);
    let mut hover = mobile("hover", MotionType::Hover, 2.0);
    hover.elevation = Some(0.5);
    let w = world(
        vec![
            mobile("tank", MotionType::Land, 2.0),
            amph,
            mobile("float", MotionType::AmphibiousFloating, 2.0),
            hover,
            mobile("boat", MotionType::Water, 2.0),
            sub,
        ],
        Vec::new(),
        wet.clone(),
    );
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    // Cells (0, 1): dry; (3, 1): depth 1/32; (6, 1): depth 0.5; (9, 1):
    // depth 1.5; (12, 1): depth 30.
    let place = |s: &mut State, blueprint: &str, cell: u32| {
        s.spawn(
            Army(1),
            blueprint,
            pos(f64::from(cell) + 0.5, 99.0, 1.5),
            q(100),
        )
    };
    let probes = [
        ("tank", 0, Layer::Land),
        ("tank", 3, Layer::Land),
        ("amph", 0, Layer::Land),
        ("amph", 6, Layer::Seabed),
        ("float", 0, Layer::Land),
        ("float", 9, Layer::Water),
        ("hover", 0, Layer::Land),
        ("hover", 3, Layer::Land),
        ("hover", 6, Layer::Water),
        ("boat", 9, Layer::Water),
        ("sub", 12, Layer::Sub),
    ];
    let ids: Vec<EntityId> = probes
        .iter()
        .map(|(b, c, _)| place(&mut s, b, *c))
        .collect();
    step(&mut s, &[]);
    for ((blueprint, cell, layer), id) in probes.iter().zip(&ids) {
        let e = s.entities.get(id).expect("exists");
        assert_eq!(e.layer, *layer, "{blueprint} in column {cell}");
        let ground = if *cell == 0 {
            5.0
        } else {
            f64::from(ZONE_HEIGHTS[(*cell / 3) as usize]) / 128.0
        };
        let want_y = match (*blueprint, *layer) {
            ("boat", _) | ("float", Layer::Water) => 4.0,
            ("sub", _) => 4.0 - 3.0,
            ("hover", Layer::Water) => 4.0 + 0.5,
            ("hover", _) => ground + 0.5,
            _ => ground,
        };
        assert_eq!(
            e.pos.y.to_bits(),
            want_y.to_bits(),
            "{blueprint} y in column {cell}"
        );
    }

    // A naval unit crosses water; a land unit's Move onto water is dropped.
    let boat = ids[9];
    let tank = ids[0];
    let before = s.counters.commands_ignored;
    step(
        &mut s,
        &[
            order_cmd(1, 0, boat, move_to(13.5, 2.5), QueueMode::Replace),
            order_cmd(1, 1, tank, move_to(10.5, 2.5), QueueMode::Replace),
        ],
    );
    assert_eq!(
        s.counters.commands_ignored,
        before + 1,
        "the land Move onto water"
    );
    assert!(s.entities.get(&tank).expect("exists").orders.is_empty());
    let mut arrived = false;
    for _ in 0..200 {
        if s.counters.moves_completed == 1 {
            arrived = true;
            break;
        }
        step(&mut s, &[]);
    }
    assert!(arrived, "the boat arrives");
    let e = s.entities.get(&boat).expect("exists");
    assert_eq!(e.layer, Layer::Water);
    assert_eq!(e.pos.y.to_bits(), 4.0_f64.to_bits());
}
