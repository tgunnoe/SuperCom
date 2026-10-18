//! A pinned digest of the movement phase (docs/specs/sim.md, SIM-065,
//! SIM-068, SIM-070; docs/specs/determinism.md): one synthetic scenario on
//! a map with water runs a turn from rest, a lattice route around a ridge
//! with diagonals and bends, an off-centre goal, braking, a naval unit, and
//! a guard that follows, and the final digest of the chain is a literal
//! computed once and written down, so a second host that runs the same
//! tests compares its binary64 geometry with this one's. All synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use supercom_sim::{
    Army, Digest, EntityId, FOLLOW_RANGE, MapGrid, MotionType, Order, QueueMode, State, UnitKind,
};
use support::{kind, order_cmd, pos, q, state, step, world};

/// The final digest of the scenario, computed once on `x86_64` from a run of
/// the scenario below and written here.
const GOLDEN: Digest = [
    109, 149, 169, 186, 50, 249, 141, 234, 99, 75, 41, 72, 132, 171, 101, 32, 168, 229, 99, 230,
    187, 106, 34, 58, 161, 193, 129, 102, 10, 55, 104, 228,
];

fn mover(id: &str, motion: MotionType, speed: f64, dynamics: Option<(f64, f64, f64)>) -> UnitKind {
    let mut k = kind(id, &["MOBILE"]);
    k.motion = motion;
    k.max_speed = Some(speed);
    if let Some((acceleration, brake, turn)) = dynamics {
        k.max_acceleration = Some(acceleration);
        k.max_brake = Some(brake);
        k.turn_rate = Some(turn);
    }
    k
}

/// A 32 by 16 map: land at height 5 on the west half, water 4 deep on the
/// east half under a surface of 4, a cliff between them, and a ridge along
/// corner column 8 from row 0 to row 10 that land units must go around.
fn map() -> MapGrid {
    let (w, h) = (32_usize, 16_usize);
    let mut samples = Vec::new();
    for z in 0..=h {
        for x in 0..=w {
            let base: i16 = if x >= 16 { 0 } else { 640 };
            samples.push(if x == 8 && z <= 10 { 1400 } else { base });
        }
    }
    MapGrid::from_heightmap_with_water(32, 16, &samples, 0.007_812_5, Some(4.0)).expect("a wet map")
}

/// Run the scenario and return the final digest and the state.
fn run() -> (Digest, State) {
    let w = world(
        vec![
            mover("tank", MotionType::Land, 3.0, Some((2.0, 4.0, 90.0))),
            mover("boat", MotionType::Water, 4.0, Some((2.0, 3.0, 60.0))),
            mover("scout", MotionType::Land, 2.0, None),
        ],
        Vec::new(),
        map(),
    );
    let mut s = state(&w, q(0), q(0));
    // The tank starts off centre facing east with the ridge between it and
    // its off-centre goal; the leader walks to a cell near the goal; the
    // boat crosses the water on a diagonal; the scout guards the leader.
    let tank: EntityId = s.spawn(Army(1), "tank", pos(2.3, 0.0, 2.9), q(100));
    let leader: EntityId = s.spawn(Army(1), "tank", pos(3.5, 0.0, 13.5), q(100));
    let boat: EntityId = s.spawn(Army(1), "boat", pos(20.5, 0.0, 4.5), q(100));
    let scout: EntityId = s.spawn(Army(1), "scout", pos(5.5, 0.0, 1.5), q(100));
    let mut digest = Digest::default();
    for tick in 0..600_u64 {
        let cmds = if tick == 0 {
            vec![
                order_cmd(
                    0,
                    0,
                    tank,
                    Order::Move {
                        to: pos(12.7, 0.0, 3.2),
                    },
                    QueueMode::Replace,
                ),
                order_cmd(
                    0,
                    1,
                    leader,
                    Order::Move {
                        to: pos(13.5, 0.0, 12.5),
                    },
                    QueueMode::Replace,
                ),
                order_cmd(
                    0,
                    2,
                    boat,
                    Order::Move {
                        to: pos(28.5, 0.0, 12.5),
                    },
                    QueueMode::Replace,
                ),
                order_cmd(
                    0,
                    3,
                    scout,
                    Order::Guard { target: leader },
                    QueueMode::Replace,
                ),
            ]
        } else {
            Vec::new()
        };
        digest = step(&mut s, &cmds);
    }
    (digest, s)
}

/// The scenario's final digest equals the pinned literal, twice over, and
/// the scenario does what its description says: the three Moves complete,
/// the scout keeps its Guard and ends within the follow range of the leader.
#[test]
fn sim_movement_golden_digest_is_pinned() {
    let (first, s) = run();
    let (second, _) = run();
    assert_eq!(first, second, "two runs agree");
    assert_eq!(s.counters.moves_completed, 3, "the three Moves complete");
    assert_eq!(s.counters.commands_ignored, 0);
    let scout = s
        .entities
        .values()
        .find(|e| e.blueprint == "scout")
        .expect("the scout");
    assert_eq!(scout.orders.len(), 1, "the Guard is kept");
    let leader = s
        .entities
        .values()
        .filter(|e| e.blueprint == "tank")
        .max_by_key(|e| e.id)
        .expect("the leader");
    let (dx, dz) = (scout.pos.x - leader.pos.x, scout.pos.z - leader.pos.z);
    assert!(
        dx * dx + dz * dz <= FOLLOW_RANGE * FOLLOW_RANGE,
        "the scout followed the leader into range"
    );
    assert_eq!(first, GOLDEN, "the pinned digest: {first:?}");
}
