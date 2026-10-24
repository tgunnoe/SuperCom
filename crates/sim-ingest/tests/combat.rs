//! Conformance of the combat rules against the installed game: a UEF T1
//! tank kills a Cybran generator on `SCMP_012`. Game-dependent: it skips,
//! loudly, without the install and fails if `SUPERCOM_REQUIRE_GAME=1`.
//! Counts and numbers only; no game content is kept in the tree.
//!
//! Test construction: both skirmish commanders are kept, because an army
//! without its commander is defeated and a defeat is not under test, but each
//! is written to a land cell at least 30 world units from every cell of the
//! flat run and from the other, which is beyond the 22 units either
//! commander's weapons reach and beyond the tank's 18, so that only the
//! tank's weapon is in play (the UEF commander's guns would otherwise reach
//! the generator from its start marker); and the tank and the generator stand
//! on a flat run of one row of the heightmap, eight cells apart, so that a
//! straight shot never meets a corner above its path.

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
    Army, Command, Digest, EntityId, Faction, Layer, MarkerKind, Order, Pos, Q, QueueMode, State,
    Tick, World,
};
use supercom_sim_ingest::load_world;

const TICKS: u64 = 3_000;

/// A row and a start column where cells `x..=x+8` are land and the nine
/// corner samples `x..=x+8` of that row share one height, the nearest to the
/// UEF start marker by Manhattan distance, then row, then column.
fn flat_run(world: &World) -> (u32, u32, Q) {
    let marker = world
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Army(1))
        .expect("the UEF start marker")
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

/// Distance at which a commander's cell lies from the flat run and from the
/// other commander: beyond every weapon of the run (22 for a commander, 18
/// for the tank) with a margin.
const CLEAR: f64 = 30.0;

/// Two passable land cells, the first and the second in row then column
/// order that lie at least [`CLEAR`] from every cell centre of the flat run
/// `x..=x+8` of row `z` and, for the second, from the first.
fn commander_cells(world: &World, at: (u32, u32, Q)) -> [(u32, u32); 2] {
    let (x, z, _) = at;
    let map = &world.map;
    let far_from_run = |cx: u32, cz: u32| {
        (0..9).all(|d| {
            let (dx, dz) = (
                f64::from(cx) - f64::from(x + d),
                f64::from(cz) - f64::from(z),
            );
            dx.hypot(dz) >= CLEAR
        })
    };
    let mut found: Vec<(u32, u32)> = Vec::new();
    'cells: for cz in 0..map.height {
        for cx in 0..map.width {
            if !map.passable(Layer::Land, cx, cz) || !far_from_run(cx, cz) {
                continue;
            }
            if found.iter().any(|&(fx, fz)| {
                (f64::from(cx) - f64::from(fx)).hypot(f64::from(cz) - f64::from(fz)) < CLEAR
            }) {
                continue;
            }
            found.push((cx, cz));
            if found.len() == 2 {
                break 'cells;
            }
        }
    }
    let first = *found.first().expect("a land cell clear of the flat run");
    let second = *found.get(1).expect("a second land cell clear of both");
    [first, second]
}

struct Outcome {
    digests: Vec<Digest>,
    shots: Vec<u64>,
    death_tick: Option<u64>,
    state: State,
    tank: EntityId,
}

fn run(world: &Arc<World>, at: (u32, u32, Q)) -> Outcome {
    let (x, z, h) = at;
    let tank_kind = world.units.get("uel0201").expect("the UEF tank");
    let plant_kind = world.units.get("urb1101").expect("the Cybran generator");
    let mut state = Arc::clone(world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a UEF and Cybran skirmish");
    let [uef_cell, cybran_cell] = commander_cells(world, at);
    for (army, (cx, cz)) in [(Army(1), uef_cell), (Army(2), cybran_cell)] {
        let commander = state
            .entities
            .values_mut()
            .find(|e| e.army == army)
            .expect("the skirmish commander");
        commander.pos = Pos {
            x: f64::from(cx) + 0.5,
            y: world.map.height_at(cx, cz).to_num::<f64>(),
            z: f64::from(cz) + 0.5,
        };
    }
    let y = h.to_num::<f64>();
    let tank = state.spawn(
        Army(1),
        &tank_kind.id,
        Pos {
            x: f64::from(x) + 0.5,
            y,
            z: f64::from(z) + 0.5,
        },
        tank_kind.max_health,
    );
    let plant = state.spawn(
        Army(2),
        &plant_kind.id,
        Pos {
            x: f64::from(x) + 8.5,
            y,
            z: f64::from(z) + 0.5,
        },
        plant_kind.max_health,
    );
    let attack = Command {
        tick: Tick(0),
        source: 0,
        seq: 0,
        army: Army(1),
        units: vec![tank],
        order: Order::Attack { target: plant },
        mode: QueueMode::Replace,
    };
    let (mut digests, mut shots, mut death_tick) = (Vec::new(), Vec::new(), None);
    for tick in 0..TICKS {
        let launched = state.next_projectile_id;
        let batch = if tick == 0 {
            vec![attack.clone()]
        } else {
            Vec::new()
        };
        digests.push(state.step(&batch).expect("a valid tick"));
        if state.next_projectile_id != launched {
            shots.push(tick);
        }
        if !state.entities.contains_key(&plant) {
            death_tick = Some(tick);
            break;
        }
    }
    Outcome {
        digests,
        shots,
        death_tick,
        state,
        tank,
    }
}

#[test]
fn conformance_combat_tank_kills_structure_on_scmp_012() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load_world(&fa, cache.path(), "SCMP_012").expect("load_world(SCMP_012)");

    // The armour table: six entries; the Commander and Structure rows as
    // SIM-022 states, converted by SIM-050's rule.
    assert_eq!(world.armour.len(), 6);
    let conv = |d: f64| Q::checked_from_num(d).expect("representable");
    for (armour, damage_type, value) in [
        ("Commander", "Normal", 1.0),
        ("Commander", "Overcharge", 0.033_333),
        ("Commander", "Deathnuke", 0.05),
        ("Structure", "Normal", 1.0),
        ("Structure", "Overcharge", 0.066_666),
        ("Structure", "Deathnuke", 0.01),
    ] {
        assert_eq!(
            world
                .armour
                .get(armour)
                .and_then(|row| row.get(damage_type))
                .copied(),
            Some(conv(value)),
            "{armour} {damage_type}"
        );
    }

    let tank = world.units.get("uel0201").expect("the UEF tank");
    let plant = world.units.get("urb1101").expect("the Cybran generator");
    let weapon = tank.weapons.first().expect("one weapon");
    assert_eq!(tank.weapons.len(), 1);
    assert_eq!(weapon.category, "Direct Fire");
    assert_eq!(
        (
            weapon.rate_of_fire,
            weapon.max_radius,
            weapon.muzzle_velocity,
            weapon.damage
        ),
        (Some(1.0), Some(18.0), Some(25.0), Q::from_num(24))
    );
    assert_eq!(plant.armor_type, "Structure");
    assert_eq!(plant.max_health, Q::from_num(600));
    assert_eq!(plant.regen_rate.unwrap_or(Q::ZERO), Q::ZERO);

    // Expected values from the loaded kinds and the test's own arithmetic.
    let rate = weapon.rate_of_fire.expect("a rate");
    let interval = (10.0 / rate).ceil() as u64;
    let multiplier = world.armour_multiplier(&plant.armor_type, &weapon.damage_type);
    assert_eq!(multiplier, Q::ONE, "Structure takes Normal at 1");
    let per_hit = (weapon.damage * multiplier).to_num::<f64>();
    let k = (plant.max_health.to_num::<f64>() / per_hit).ceil() as u64;
    let speed = weapon.muzzle_velocity.expect("a speed");
    let (step, mut distance, mut phases) = (speed * 0.1, 8.0_f64, 1_u64);
    while distance > step + 0.5 {
        distance -= step;
        phases += 1;
    }

    let at = flat_run(&world);
    let first = run(&world, at);
    let state = &first.state;
    let first_shot = *first.shots.first().expect("the tank fired");
    assert_eq!(first_shot, 0, "the first shot is on the first tick");
    assert!(
        first.shots.windows(2).all(|w| w[1] - w[0] == interval),
        "{:?}",
        first.shots
    );
    let expected_death = first_shot + interval * (k - 1) + (phases - 1);
    assert_eq!(
        first.death_tick,
        Some(expected_death),
        "dies on its k-th impact, k = {k}"
    );
    assert!(state.counters.projectile_impacts >= k);
    assert_eq!(state.counters.deaths, 1);
    assert_eq!(state.counters.defeats, 0, "both commanders stand");
    assert!(
        state
            .entities
            .get(&first.tank)
            .expect("the tank")
            .orders
            .is_empty()
    );

    let second = run(&world, at);
    assert_eq!(
        first.digests, second.digests,
        "two identical runs agree at every tick"
    );
}
