//! Conformance of the movement rules against the installed game: a UEF T1
//! tank crosses `SCMP_012`, and a naval unit crosses the water of
//! `SCMP_037`. Game-dependent: each test skips, loudly, without the install
//! and fails if `SUPERCOM_REQUIRE_GAME=1`. Counts and numbers only; no game
//! content is kept in the tree.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use supercom_sim::{
    ARRIVAL_RADIUS, Army, Command, Digest, EntityId, Faction, Layer, MotionType, Order, Pos,
    QueueMode, State, Tick, UnitKind, World, plan_route,
};
use supercom_sim_ingest::load_world;

/// Water-passable depth, 1.5, in the heightmap's raw units of 2^-7.
const WATER_DEPTH_RAW: i64 = 192;

/// How many corner samples of `SCMP_037` lie at least 1.5 under its water
/// surface of 17.5, counted from the map file's own samples by this test.
const SCMP_037_WATER_CELLS: usize = 60_600;

fn index(width: u32, x: u32, z: u32) -> usize {
    (z as usize) * (width as usize) + (x as usize)
}

fn cell_of(p: &Pos) -> (u32, u32) {
    assert!(
        p.x >= 0.0 && p.z >= 0.0 && p.x < 1.0e6 && p.z < 1.0e6,
        "{p:?} is inside a map"
    );
    (
        format!("{:.0}", p.x.floor()).parse().expect("a cell"),
        format!("{:.0}", p.z.floor()).parse().expect("a cell"),
    )
}

fn centre(cell: (u32, u32)) -> Pos {
    Pos {
        x: f64::from(cell.0) + 0.5,
        y: 0.0,
        z: f64::from(cell.1) + 0.5,
    }
}

/// The cells of Chebyshev ring `r` around `c` inside a `w` by `h` grid, in
/// ascending index.
fn ring(c: (u32, u32), r: u32, w: u32, h: u32) -> Vec<(u32, u32)> {
    let (cx, cz) = (i64::from(c.0), i64::from(c.1));
    let r = i64::from(r);
    let mut cells = Vec::new();
    for z in cz - r..=cz + r {
        for x in cx - r..=cx + r {
            if (x - cx).abs().max((z - cz).abs()) == r
                && x >= 0
                && z >= 0
                && x < i64::from(w)
                && z < i64::from(h)
            {
                cells.push((u32::try_from(x).expect("x"), u32::try_from(z).expect("z")));
            }
        }
    }
    cells
}

fn spawn_and_command(
    state: &mut State,
    army: u8,
    id: &str,
    at: Pos,
    health: supercom_sim::Q,
    to: Pos,
) -> (EntityId, Command) {
    let e = state.spawn(Army(army), id, at, health);
    let command = Command {
        tick: Tick(0),
        source: 0,
        seq: 0,
        army: Army(army),
        units: vec![e],
        order: Order::Move { to },
        mode: QueueMode::Replace,
    };
    (e, command)
}

fn first_kind<'w>(world: &'w World, prefer: &str, motion: MotionType) -> &'w UnitKind {
    world
        .units
        .get(prefer)
        .filter(|k| k.motion == motion && k.max_speed.is_some())
        .or_else(|| {
            world
                .units
                .values()
                .find(|k| k.motion == motion && k.max_speed.is_some())
        })
        .expect("a kind of the motion type")
}

// ---- the land unit on SCMP_012 ------------------------------------------------

/// One run: the UEF tank at the UEF commander's cell moves to the land cell
/// nearest the point 40 units toward the map's centre. Returns the digest
/// after each tick.
fn run_land(world: &Arc<World>) -> Vec<Digest> {
    let mut state = Arc::clone(world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a UEF and Cybran skirmish");
    let commander = state
        .entities
        .values()
        .find(|e| e.army == Army(1))
        .expect("the UEF commander")
        .pos;
    let kind = first_kind(world, "uel0201", MotionType::Land);
    let (w, h) = (world.map.width, world.map.height);
    // The point 40 units from the commander toward the map's centre.
    let (cx, cz) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
    let (dx, dz) = (cx - commander.x, cz - commander.z);
    let len = (dx * dx + dz * dz).sqrt();
    let point = (commander.x + 40.0 * dx / len, commander.z + 40.0 * dz / len);
    let base = cell_of(&Pos {
        x: point.0,
        y: 0.0,
        z: point.1,
    });
    // Outward by Chebyshev ring, then by squared distance and index.
    let goal = (0..w.max(h))
        .find_map(|r| {
            let mut open: Vec<((u32, u32), f64)> = ring(base, r, w, h)
                .into_iter()
                .filter(|c| world.map.passable(Layer::Land, c.0, c.1))
                .map(|c| {
                    let m = centre(c);
                    (c, (m.x - point.0).powi(2) + (m.z - point.1).powi(2))
                })
                .collect();
            open.sort_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then(index(w, a.0.0, a.0.1).cmp(&index(w, b.0.0, b.0.1)))
            });
            open.first().map(|c| c.0)
        })
        .expect("a land cell near the point");
    let start = cell_of(&commander);
    let (tank, command) = spawn_and_command(
        &mut state,
        1,
        &kind.id,
        commander,
        kind.max_health,
        centre(goal),
    );

    // The route the planner gives over the loaded grid, by the test's own
    // run: it exists, it ends at the goal, and it is at least as long as the
    // straight-line cell distance and not absurdly longer.
    let planned = plan_route(&world.map, Layer::Land, start, goal).unwrap_or_default();
    let straight = usize::try_from(
        (i64::from(goal.0) - i64::from(start.0))
            .abs()
            .max((i64::from(goal.1) - i64::from(start.1)).abs()),
    )
    .expect("small");
    assert_eq!(
        planned.last(),
        Some(&goal),
        "the planner reaches the goal cell"
    );
    assert!(
        planned.len() >= straight && planned.len() <= 3 * straight + 8,
        "{} cells for {straight}",
        planned.len()
    );

    let mut digests = Vec::new();
    let mut cells = BTreeSet::new();
    let mut done = false;
    for tick in 0..2000_u64 {
        let batch = if tick == 0 {
            vec![command.clone()]
        } else {
            Vec::new()
        };
        digests.push(state.step(&batch).expect("a valid tick"));
        let e = state.entities.get(&tank).expect("the tank exists");
        if tick == 0 {
            assert_eq!(
                e.route[..],
                planned[planned.len() - e.route.len()..],
                "the route is the planned one"
            );
        }
        let cell = cell_of(&e.pos);
        assert!(
            world.map.passable(Layer::Land, cell.0, cell.1),
            "tick {tick}: {cell:?} is land"
        );
        cells.insert(cell);
        if state.counters.moves_completed == 1 {
            done = true;
            break;
        }
    }
    assert!(done, "the Move completes within 2000 ticks");
    let e = state.entities.get(&tank).expect("the tank exists");
    let target = centre(goal);
    let (ex, ez) = (e.pos.x - target.x, e.pos.z - target.z);
    assert!(ex * ex + ez * ez <= ARRIVAL_RADIUS * ARRIVAL_RADIUS);
    assert_eq!(state.counters.moves_completed, 1);
    assert!(e.orders.is_empty() && e.route.is_empty() && e.goal.is_none());
    assert!(
        cells.len() >= straight,
        "the tank crossed {} cells",
        cells.len()
    );
    digests
}

#[test]
fn conformance_movement_land_unit_crosses_scmp_012() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load_world(&fa, cache.path(), "SCMP_012").expect("SCMP_012 loads");
    let first = run_land(&world);
    let second = run_land(&world);
    assert_eq!(first, second, "two identical runs agree at every tick");
}

// ---- the naval unit on SCMP_037 ------------------------------------------------

/// The `.scmap` of `folder` parsed, from the file whose name ends `.scmap`.
fn read_map(fa: &Path, folder: &str) -> supercom_scmap::Map {
    let dir = fa.join("maps").join(folder);
    let path = fs::read_dir(&dir)
        .expect("the map folder lists")
        .map(|e| e.expect("an entry").path())
        .find(|p| p.to_string_lossy().to_lowercase().ends_with(".scmap"))
        .expect("a .scmap file");
    supercom_scmap::Map::parse(&fs::read(path).expect("the map file")).expect("the map parses")
}

#[test]
fn conformance_movement_naval_unit_crosses_scmp_037() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load_world(&fa, cache.path(), "SCMP_037").expect("SCMP_037 loads");
    let file = read_map(&fa, "SCMP_037");
    let water = file.water.expect("SCMP_037 has water");
    assert_eq!(water.elevation.to_bits(), 17.5_f32.to_bits());
    let (w, h) = (world.map.width, world.map.height);
    let samples = &file.heightmap.samples;
    // The surface 17.5 is 2240 raw units; a cell's depth is the surface less
    // its corner sample.
    let deep = |x: u32, z: u32| 2240 - i64::from(samples[index(w + 1, x, z)]) >= WATER_DEPTH_RAW;
    let mut count = 0;
    for z in 0..h {
        for x in 0..w {
            count += usize::from(deep(x, z));
        }
    }
    assert_eq!(
        count, SCMP_037_WATER_CELLS,
        "water-passable cells by the file's own heights"
    );
    let grid = (0..h)
        .flat_map(|z| (0..w).map(move |x| (x, z)))
        .filter(|c| world.map.passable(Layer::Water, c.0, c.1))
        .count();
    assert_eq!(grid, count, "the loaded water grid agrees");

    // A water-passable start nearest the centre, and the nearest reachable
    // cell (by 4-connected flood fill) at least 30 units from it.
    let centre_cell = (w / 2, h / 2);
    let start = (0..w)
        .find_map(|r| {
            ring(centre_cell, r, w, h)
                .into_iter()
                .find(|c| deep(c.0, c.1))
        })
        .expect("a water cell near the centre");
    let mut seen = BTreeSet::from([start]);
    let mut frontier = vec![start];
    while let Some((x, z)) = frontier.pop() {
        for (nx, nz) in [
            (x.wrapping_sub(1), z),
            (x + 1, z),
            (x, z.wrapping_sub(1)),
            (x, z + 1),
        ] {
            if nx < w && nz < h && deep(nx, nz) && seen.insert((nx, nz)) {
                frontier.push((nx, nz));
            }
        }
    }
    let far = |c: &(u32, u32)| {
        (f64::from(c.0) - f64::from(start.0)).powi(2)
            + (f64::from(c.1) - f64::from(start.1)).powi(2)
    };
    let goal = seen
        .iter()
        .copied()
        .filter(|c| far(c) >= 900.0)
        .min_by(|a, b| {
            far(a)
                .total_cmp(&far(b))
                .then(index(w, a.0, a.1).cmp(&index(w, b.0, b.1)))
        })
        .expect("a reachable water cell 30 units away");

    let mut state = Arc::clone(&world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a skirmish");
    let boat_kind = first_kind(&world, "", MotionType::Water);
    let (boat, command) = spawn_and_command(
        &mut state,
        1,
        &boat_kind.id,
        centre(start),
        boat_kind.max_health,
        centre(goal),
    );
    let mut done = false;
    for tick in 0..3000_u64 {
        let batch = if tick == 0 {
            vec![command.clone()]
        } else {
            Vec::new()
        };
        state.step(&batch).expect("a valid tick");
        let e = state.entities.get(&boat).expect("the boat exists");
        let cell = cell_of(&e.pos);
        assert!(deep(cell.0, cell.1), "tick {tick}: {cell:?} is water");
        if state.counters.moves_completed == 1 {
            done = true;
            break;
        }
    }
    assert!(done, "the boat arrives within 3000 ticks");
    let e = state.entities.get(&boat).expect("the boat exists");
    assert_eq!(e.layer, Layer::Water);
    assert_eq!(e.pos.y.to_bits(), 17.5_f64.to_bits());
    let target = centre(goal);
    let (ex, ez) = (e.pos.x - target.x, e.pos.z - target.z);
    assert!(ex * ex + ez * ez <= ARRIVAL_RADIUS * ARRIVAL_RADIUS);

    // A land unit ordered onto that water has its Move dropped.
    let mut state = Arc::clone(&world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a skirmish");
    let commander = state
        .entities
        .values()
        .find(|e| e.army == Army(1))
        .expect("the UEF commander")
        .pos;
    let tank_kind = first_kind(&world, "uel0201", MotionType::Land);
    let (tank, command) = spawn_and_command(
        &mut state,
        1,
        &tank_kind.id,
        commander,
        tank_kind.max_health,
        centre(goal),
    );
    state.step(&[command]).expect("a valid tick");
    assert!(
        state
            .entities
            .get(&tank)
            .expect("the tank")
            .orders
            .is_empty()
    );
    assert_eq!(state.counters.commands_ignored, 1);
    assert_eq!(state.counters.moves_completed, 0);
}

// ---- the floating unit crossing a shore on SCMP_037 ------------------------------

/// A floating kind (an engineer of the amphibious-floating motion type)
/// ordered from its commander's land cell to the nearest water cell arrives;
/// the union of the land layer and the water layer has no step across a
/// gradual shore, so under it the Move is dropped (SIM-070).
#[test]
fn conformance_movement_floating_unit_crosses_a_shore_on_scmp_037() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load_world(&fa, cache.path(), "SCMP_037").expect("SCMP_037 loads");
    let kind = world.units.get("uel0105").expect("uel0105 is installed");
    assert_eq!(kind.motion, MotionType::AmphibiousFloating);
    let mut state = Arc::clone(&world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a skirmish");
    let commander = state
        .entities
        .values()
        .find(|e| e.army == Army(1))
        .expect("the UEF commander")
        .pos;
    let from = cell_of(&commander);
    assert!(
        world.map.passable(Layer::Land, from.0, from.1),
        "the commander stands on land"
    );
    let (w, h) = (world.map.width, world.map.height);
    // The nearest water-passable cell by the distance between cell centres,
    // ties by ascending index.
    let distance = |c: &(u32, u32)| {
        let (dx, dz) = (
            i64::from(c.0) - i64::from(from.0),
            i64::from(c.1) - i64::from(from.1),
        );
        dx * dx + dz * dz
    };
    let goal = (0..h)
        .flat_map(|z| (0..w).map(move |x| (x, z)))
        .filter(|c| world.map.passable(Layer::Water, c.0, c.1))
        .min_by_key(|c| (distance(c), index(w, c.0, c.1)))
        .expect("a water cell");
    let (unit, command) = spawn_and_command(
        &mut state,
        1,
        &kind.id,
        commander,
        kind.max_health,
        centre(goal),
    );
    let mut arrived = false;
    for tick in 0..3000_u64 {
        let batch = if tick == 0 {
            vec![command.clone()]
        } else {
            Vec::new()
        };
        state.step(&batch).expect("a valid tick");
        if state.counters.moves_completed == 1 {
            arrived = true;
            break;
        }
    }
    assert_eq!(state.counters.commands_ignored, 0, "the Move is planned");
    assert!(arrived, "the engineer arrives within 3000 ticks");
    let e = state.entities.get(&unit).expect("the engineer exists");
    assert_eq!(e.layer, Layer::Water);
    let target = centre(goal);
    let (ex, ez) = (e.pos.x - target.x, e.pos.z - target.z);
    assert!(ex * ex + ez * ez <= ARRIVAL_RADIUS * ARRIVAL_RADIUS);
}
