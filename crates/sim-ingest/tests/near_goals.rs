//! No ground or naval mover circles a near goal (docs/specs/sim.md,
//! SIM-065): every installed mover whose motion type is not Air is sent
//! five near goals from rest and two at speed, and must arrive within 1,200
//! ticks without a Move being dropped. Game-dependent: each test skips,
//! loudly, without the install and fails if `SUPERCOM_REQUIRE_GAME=1`.
//! Counts and identifiers only; no game content is kept in the tree.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::sync::Arc;

use supercom_sim::{
    Army, Command, EntityId, Faction, Layer, MotionType, Order, Pos, QueueMode, State, Tick,
    UnitKind, World,
};
use supercom_sim_ingest::load_world;

/// How many ticks a case may take before it counts as circling.
const LIMIT: u64 = 1_200;

fn at(x: f64, z: f64) -> Pos {
    Pos { x, y: 0.0, z }
}

fn command(tick: u64, seq: u32, unit: EntityId, to: Pos) -> Command {
    Command {
        tick: Tick(tick),
        source: 0,
        seq,
        army: Army(1),
        units: vec![unit],
        order: Order::Move { to },
        mode: QueueMode::Replace,
    }
}

/// One case: spawn `kind` at `start`, Move to `first`; at tick `switch`, if
/// any, Replace it with a Move to `second`. `Ok` on the queue emptying
/// after the last Move, `Err` with the reason otherwise.
fn trial(
    base: &State,
    kind: &UnitKind,
    start: Pos,
    first: Pos,
    second: Option<(u64, Pos)>,
) -> Result<(), String> {
    let mut state = base.clone();
    let unit = state.spawn(Army(1), &kind.id, start, kind.max_health);
    let ignored = state.counters.commands_ignored;
    for tick in 0..LIMIT {
        let mut batch = Vec::new();
        if tick == 0 {
            batch.push(command(0, 0, unit, first));
        }
        if let Some((when, to)) = second
            && tick == when
        {
            batch.push(command(tick, 1, unit, to));
        }
        state.step(&batch).map_err(|e| format!("{e:?}"))?;
        if state.counters.commands_ignored > ignored {
            return Err("a Move was dropped".into());
        }
        let idle = state
            .entities
            .get(&unit)
            .is_some_and(|e| e.orders.is_empty());
        if idle && second.is_none_or(|(when, _)| tick > when) {
            return Ok(());
        }
    }
    Err(format!("no arrival in {LIMIT} ticks"))
}

/// The kinds of `world` with a speed whose motion type is among `motions`.
fn movers<'w>(world: &'w World, motions: &[MotionType]) -> Vec<&'w UnitKind> {
    world
        .units
        .values()
        .filter(|k| k.max_speed.is_some() && motions.contains(&k.motion))
        .collect()
}

/// Run the seven cases for every kind; the identifiers of the kinds that
/// fail a case, with the case.
fn failing(base: &State, start: Pos, kinds: &[&UnitKind]) -> Vec<String> {
    let mut failures = Vec::new();
    for kind in kinds {
        let mut reasons = Vec::new();
        for (dx, dz) in [
            (-1.0, 0.0),
            (0.0, 3.0),
            (2.0, 2.0),
            (-3.0, -3.0),
            (10.0, 0.0),
        ] {
            if let Err(why) = trial(base, kind, start, at(start.x + dx, start.z + dz), None) {
                reasons.push(format!("rest to ({dx}, {dz}): {why}"));
            }
        }
        // At speed: head along +x for 15 ticks, then turn back.
        for (dx, dz) in [(-2.0, 0.0), (0.0, 1.0)] {
            let far = at(start.x + 12.0, start.z);
            let back = at(start.x + dx, start.z + dz);
            if let Err(why) = trial(base, kind, start, far, Some((15, back))) {
                reasons.push(format!("at speed to ({dx}, {dz}): {why}"));
            }
        }
        if !reasons.is_empty() {
            failures.push(format!(
                "{} {:?}: {}",
                kind.id,
                kind.motion,
                reasons.join("; ")
            ));
        }
    }
    failures
}

#[test]
fn conformance_ground_movers_reach_near_goals_on_scmp_012() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load_world(&fa, cache.path(), "SCMP_012").expect("SCMP_012 loads");
    let base = Arc::clone(&world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a UEF and Cybran skirmish");
    let commander = base
        .entities
        .values()
        .find(|e| e.army == Army(1))
        .expect("the UEF commander")
        .pos;
    let start = at(commander.x.floor() + 0.5, commander.z.floor() + 0.5);
    let kinds = movers(
        &world,
        &[
            MotionType::Land,
            MotionType::Amphibious,
            MotionType::AmphibiousFloating,
            MotionType::Hover,
        ],
    );
    assert!(kinds.len() > 50, "{} ground kinds", kinds.len());
    let failures = failing(&base, start, &kinds);
    assert!(
        failures.is_empty(),
        "{} of {} kinds circle or drop a near goal:\n{}",
        failures.len(),
        kinds.len(),
        failures.join("\n")
    );
}

#[test]
fn conformance_naval_movers_reach_near_goals_on_scmp_037() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load_world(&fa, cache.path(), "SCMP_037").expect("SCMP_037 loads");
    let base = Arc::clone(&world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a UEF and Cybran skirmish");
    // The first cell, by growing distance from the map's centre, with open
    // water and sub passability over the whole span the cases cross.
    let (w, h) = (world.map.width, world.map.height);
    let clear = |x: u32, z: u32| {
        (x.saturating_sub(4)..=x + 14).all(|xx| {
            (z.saturating_sub(4)..=z + 4).all(|zz| {
                xx < w
                    && zz < h
                    && world.map.passable(Layer::Water, xx, zz)
                    && world.map.passable(Layer::Sub, xx, zz)
            })
        })
    };
    let cell = (0..w.max(h))
        .flat_map(|r| {
            let (cx, cz) = (w / 2, h / 2);
            (cz.saturating_sub(r)..=(cz + r).min(h - 1)).flat_map(move |z| {
                (cx.saturating_sub(r)..=(cx + r).min(w - 1)).map(move |x| (x, z))
            })
        })
        .find(|&(x, z)| x >= 4 && z >= 4 && clear(x, z))
        .expect("an open stretch of water");
    let start = at(f64::from(cell.0) + 0.5, f64::from(cell.1) + 0.5);
    let kinds = movers(&world, &[MotionType::Water, MotionType::SurfacingSub]);
    assert!(!kinds.is_empty(), "no naval kinds");
    let failures = failing(&base, start, &kinds);
    assert!(
        failures.is_empty(),
        "{} of {} kinds circle or drop a near goal:\n{}",
        failures.len(),
        kinds.len(),
        failures.join("\n")
    );
}
