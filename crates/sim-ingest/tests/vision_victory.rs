//! Conformance of the vision and victory rules against the installed game
//! on `SCMP_012`: game-dependent, it skips, loudly, without the install and
//! fails if `SUPERCOM_REQUIRE_GAME=1`. Counts and numbers only; no game
//! content is kept in the tree.
//!
//! Measured on the install: the UEF and Cybran start markers of `SCMP_012`
//! lie 321.03 world units apart on the horizontal plane, and the UEF and
//! Cybran commanders' vision radii are 26 and 26, both far below that, so
//! after one tick each commander sees only itself.
//!
//! Test construction: the Cybran commander is moved onto a flat run of land
//! nearest its start marker, its health set to 1, and a UEF tank, the
//! combat conformance's kind, is placed 8 units west of it on the
//! same run, inside the tank's weapon range, with an Attack on the
//! commander, so that a straight shell meets no terrain corner (the combat
//! conformance's construction); the first impact kills it.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::sync::Arc;

use supercom_sim::{
    Army, Command, Digest, Faction, Layer, MarkerKind, Order, Outcome, Pos, Q, QueueMode, State,
    Tick, World, distance_xz,
};
use supercom_sim_ingest::load_world;

const AFTER_OUTCOME: u64 = 50;
const GIVE_UP: u64 = 200;
const TANK_OFFSET: f64 = 8.0;

struct Run {
    digests: Vec<Digest>,
    death_tick: u64,
    first_impact_tick: u64,
    state: State,
}

/// A row and a start column where cells `x..=x+8` are land and the nine
/// corner samples `x..=x+8` of that row share one height, the nearest to the
/// Cybran start marker by Manhattan distance, then row, then column.
fn flat_run(world: &World) -> (u32, u32, Q) {
    let marker = world
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Army(2))
        .expect("the Cybran start marker")
        .pos;
    let map = &world.map;
    let mut best: Option<((i64, u32, u32), Q)> = None;
    for z in 0..map.height {
        for x in 0..map.width.saturating_sub(9) {
            let h = map.height_at(x, z);
            let flat =
                (x..x + 9).all(|c| map.height_at(c, z) == h && map.passable(Layer::Land, c, z));
            if !flat {
                continue;
            }
            let near =
                (f64::from(x) + 4.5 - marker.x).abs() + (f64::from(z) + 0.5 - marker.z).abs();
            let key = (near.round() as i64, z, x);
            if best.as_ref().is_none_or(|(k, _)| key < *k) {
                best = Some((key, h));
            }
        }
    }
    let ((_, z, x), h) = best.expect("a flat run of land near the start marker");
    (x, z, h)
}

fn run(world: &Arc<World>) -> Run {
    let mut state = Arc::clone(world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a UEF and Cybran skirmish");
    let (uef, cybran) = (Army(1), Army(2));
    let target = state
        .entities
        .values()
        .find(|e| e.army == cybran)
        .expect("the Cybran commander")
        .id;
    let (x, z, h) = flat_run(world);
    let y = h.to_num::<f64>();
    let at = Pos {
        x: f64::from(x) + 8.5,
        y,
        z: f64::from(z) + 0.5,
    };
    {
        let cmd = state.entities.get_mut(&target).expect("the commander");
        cmd.health = Q::from_num(1);
        cmd.pos = at;
    }
    let tank_kind = world.units.get("uel0201").expect("the UEF tank");
    let tank = state.spawn(
        uef,
        &tank_kind.id,
        Pos {
            x: at.x - TANK_OFFSET,
            y,
            z: at.z,
        },
        tank_kind.max_health,
    );
    let attack = Command {
        tick: Tick(0),
        source: 0,
        seq: 0,
        army: uef,
        units: vec![tank],
        order: Order::Attack { target },
        mode: QueueMode::Replace,
    };
    let mut digests = Vec::new();
    let (mut death_tick, mut first_impact_tick) = (None, None);
    for tick in 0..GIVE_UP {
        let batch = if tick == 0 {
            vec![attack.clone()]
        } else {
            Vec::new()
        };
        digests.push(state.step(&batch).expect("a valid tick"));
        if first_impact_tick.is_none() && state.counters.projectile_impacts > 0 {
            first_impact_tick = Some(tick);
        }
        if !state.entities.contains_key(&target) {
            death_tick = Some(tick);
            break;
        }
    }
    let death_tick = death_tick.expect("the commander dies");
    for _ in 0..AFTER_OUTCOME {
        let batch: &[Command] = &[];
        digests.push(state.step(batch).expect("a valid tick"));
    }
    Run {
        digests,
        death_tick,
        first_impact_tick: first_impact_tick.expect("a shell landed"),
        state,
    }
}

#[test]
fn conformance_vision_and_victory_on_scmp_012() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load_world(&fa, cache.path(), "SCMP_012").expect("load_world(SCMP_012)");

    let marker = |slot| {
        world
            .markers
            .iter()
            .find(|m| m.kind == MarkerKind::Army(slot))
            .expect("an army marker")
            .pos
    };
    let apart = distance_xz(&marker(1), &marker(2));
    let radius = |id: &str| {
        world
            .units
            .get(id)
            .and_then(|k| k.vision_radius)
            .expect("a commander with a vision radius")
    };
    let (uef_radius, cybran_radius) = (radius("uel0001"), radius("url0001"));
    assert!((321.0..321.1).contains(&apart), "marker distance {apart}");
    assert_eq!((uef_radius, cybran_radius), (26.0, 26.0));
    assert!(apart > uef_radius.max(cybran_radius));

    // After one tick each commander sees itself and not the other.
    let mut state = Arc::clone(&world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a skirmish");
    state.step(&[]).expect("a valid tick");
    let ids: Vec<_> = state.entities.keys().copied().collect();
    let [a, b] = ids[..] else {
        panic!("two commanders, got {ids:?}");
    };
    assert!(state.is_visible(Army(1), a) && !state.is_visible(Army(1), b));
    assert!(state.is_visible(Army(2), b) && !state.is_visible(Army(2), a));
    assert_eq!(state.outcome, None);

    let first = run(&world);
    let s = &first.state;
    assert_eq!(
        first.death_tick, first.first_impact_tick,
        "the first impact kills"
    );
    assert!(s.armies[&Army(2)].defeated.is_some());
    assert!(s.armies[&Army(1)].defeated.is_none());
    assert_eq!(s.outcome, Some(Outcome::Winner(Army(1))));
    assert_eq!((s.counters.victories, s.counters.defeats), (1, 1));
    let second = run(&world);
    assert_eq!(
        first.digests, second.digests,
        "two identical runs agree at every tick"
    );
}
