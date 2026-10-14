//! The still-turning test of the integration rule (docs/specs/sim.md,
//! SIM-065): a unit sheds speed for its heading only on a tick whose turn
//! was clamped, and on any other tick its heading is set to the desired
//! heading exactly, so rounding dust never brakes it. Every existing
//! integration case starts on an axis-aligned cell centre; these start off
//! centre, where the heading sum leaves a residue near 5e-18. All
//! synthetic; each expected value is restated here from the clause.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::collections::BTreeMap;
use std::f64::consts::PI;

use supercom_sim::{
    ARRIVAL_RADIUS, Army, Layer, MapGrid, MotionType, Order, Q, QueueMode, UnitKind, cell_centre,
    plan_route,
};
use support::{kind, order_cmd, pos, q, state, step, world};

const DT: f64 = 0.1;

fn mover(id: &str, turn_rate: Option<f64>) -> UnitKind {
    let mut k = kind(id, &["MOBILE"]);
    k.motion = MotionType::Land;
    k.max_speed = Some(3.0);
    k.max_acceleration = Some(3.0);
    k.max_brake = Some(3.0);
    k.turn_rate = turn_rate;
    k
}

fn open_grid(size: u32) -> MapGrid {
    let cells = (size * size) as usize;
    let mut passable = BTreeMap::new();
    passable.insert(Layer::Land, vec![true; cells]);
    passable.insert(Layer::Air, vec![true; cells]);
    MapGrid {
        width: size,
        height: size,
        heights: vec![Q::ZERO; ((size + 1) * (size + 1)) as usize],
        passable,
        water: None,
    }
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

/// The rule of SIM-065 for a kind with acceleration 3, brake 3, and maximum
/// speed 3: one tick of `(x, z, heading, speed)` along `route` to `goal`;
/// the new state and whether the turn was clamped.
fn reference(
    s: (f64, f64, f64, f64),
    route: &[(u32, u32)],
    goal: (f64, f64),
    turn_rate: Option<f64>,
) -> ((f64, f64, f64, f64), bool) {
    let (x, z, heading, speed) = s;
    let (tx, tz) = route.first().map_or(goal, |c| cell_centre(*c));
    let (dx, dz) = (tx - x, tz - z);
    let desired = libm::atan2(dz, dx);
    let delta = wrap(desired - heading);
    let (new_heading, clamped) = match turn_rate {
        Some(rate) => {
            let max_turn = rate * (PI / 180.0) * DT;
            if delta.abs() > max_turn {
                (wrap(heading + delta.max(-max_turn).min(max_turn)), true)
            } else {
                (desired, false)
            }
        }
        None => (desired, false),
    };
    let mut remaining = 0.0_f64;
    let mut from = (x, z);
    for to in route
        .iter()
        .map(|c| cell_centre(*c))
        .chain(core::iter::once(goal))
    {
        let (ddx, ddz) = (to.0 - from.0, to.1 - from.1);
        remaining += libm::sqrt(ddx * ddx + ddz * ddz);
        from = to;
    }
    let new_speed = if clamped || remaining <= speed * speed / (2.0 * 3.0) {
        (speed - 3.0 * DT).max(0.0)
    } else {
        3.0_f64.min(speed + 3.0 * DT)
    };
    let stride = new_speed * DT;
    let (nx, nz) = if route.is_empty() && stride >= libm::sqrt(dx * dx + dz * dz) {
        goal
    } else {
        (
            x + stride * libm::cos(new_heading),
            z + stride * libm::sin(new_heading),
        )
    };
    ((nx, nz, new_heading, new_speed), clamped)
}

/// Run the kind from an off-centre start and compare position, heading, and
/// speed with the restatement on every tick until arrival; returns the
/// number of ticks whose speed fell and the number whose turn was clamped.
fn run(turn_rate: Option<f64>, start: (f64, f64), goal: (f64, f64)) -> (u32, u32, u32) {
    let k = mover("tank", turn_rate);
    let w = world(vec![k], Vec::new(), open_grid(64));
    let mut st = state(&w, Q::ZERO, Q::ZERO);
    let e = st.spawn(Army(1), "tank", pos(start.0, 0.0, start.1), q(100));
    let from = (start.0.floor() as u32, start.1.floor() as u32);
    let to = (goal.0.floor() as u32, goal.1.floor() as u32);
    let mut route = plan_route(&w.map, Layer::Land, from, to).expect("a route");
    let mut body = (start.0, start.1, 0.0_f64, 0.0_f64);
    let (mut slowed, mut clamps, mut ticks) = (0, 0, 0);
    for tick in 0..2_000_u32 {
        let cmds = if tick == 0 {
            vec![order_cmd(
                0,
                0,
                e,
                Order::Move {
                    to: pos(goal.0, 0.0, goal.1),
                },
                QueueMode::Replace,
            )]
        } else {
            Vec::new()
        };
        step(&mut st, &cmds);
        let (next, clamped) = reference(body, &route, goal, turn_rate);
        if next.3 < body.3 {
            slowed += 1;
        }
        clamps += u32::from(clamped);
        body = next;
        while let Some(c) = route.first() {
            let (cx, cz) = cell_centre(*c);
            let (ddx, ddz) = (cx - body.0, cz - body.1);
            if ddx * ddx + ddz * ddz <= ARRIVAL_RADIUS * ARRIVAL_RADIUS {
                route.remove(0);
            } else {
                break;
            }
        }
        let got = st.entities.get(&e).expect("the entity exists");
        ticks = tick;
        let (gx, gz) = (goal.0 - body.0, goal.1 - body.1);
        if gx * gx + gz * gz <= ARRIVAL_RADIUS * ARRIVAL_RADIUS {
            assert_eq!(st.counters.moves_completed, 1, "arrival at tick {tick}");
            break;
        }
        assert_eq!(got.pos.x.to_bits(), body.0.to_bits(), "x at tick {tick}");
        assert_eq!(got.pos.z.to_bits(), body.1.to_bits(), "z at tick {tick}");
        assert_eq!(
            got.heading.to_bits(),
            body.2.to_bits(),
            "heading at tick {tick}"
        );
        assert_eq!(
            got.speed.to_bits(),
            body.3.to_bits(),
            "speed at tick {tick}"
        );
    }
    assert_eq!(st.counters.moves_completed, 1, "the move completes");
    (slowed, clamps, ticks)
}

/// A turn-limited tank starting off centre brakes only where the turn is
/// clamped or the braking distance is reached: along a long straight route
/// every slowed tick is one of those, so the restated rule and the engine
/// agree bit for bit.
#[test]
fn sim_turning_brakes_only_on_a_clamped_step() {
    let (slowed, clamps, _) = run(Some(90.0), (2.3, 2.9), (60.5, 2.5));
    assert!(clamps >= 1, "the start faces away from the route");
    assert!(
        slowed > clamps,
        "the final approach brakes beyond the clamped ticks"
    );
}

/// A kind without a turn rate turns at once and is never still turning: its
/// speed falls only inside the braking distance, from any start.
#[test]
fn sim_a_kind_without_a_turn_rate_is_never_still_turning() {
    let (slowed, clamps, _) = run(None, (2.3, 2.9), (60.5, 40.5));
    assert_eq!(clamps, 0);
    assert!(slowed > 0, "the final approach brakes");
}
