//! The route planner (docs/specs/sim.md, SIM-066 and SIM-067): shortest
//! routes without corner cutting, and ties broken by ascending cell index.
//! Synthetic grids; expected routes are stated by construction or computed
//! by planners written here, never by the code under test.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use supercom_sim::{Layer, MapGrid, Q, plan_route};

type Cell = (u32, u32);

/// A grid whose land layer is passable exactly where `open` says, row by
/// row.
fn grid_of(width: u32, height: u32, open: impl Fn(u32, u32) -> bool) -> MapGrid {
    let mut land = Vec::new();
    for z in 0..height {
        for x in 0..width {
            land.push(open(x, z));
        }
    }
    let cells = land.len();
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

fn open_at(grid: &MapGrid, x: i64, z: i64) -> bool {
    match (u32::try_from(x), u32::try_from(z)) {
        (Ok(x), Ok(z)) => grid.passable(Layer::Land, x, z),
        _ => false,
    }
}

fn signed(v: u32) -> i64 {
    i64::from(v)
}

/// The cost of one step of a route: 10 orthogonal, 14 diagonal.
fn step_cost(a: Cell, b: Cell) -> u32 {
    if a.0 != b.0 && a.1 != b.1 { 14 } else { 10 }
}

/// Whether `route` from `from` is a chain of adjacent open cells in which
/// no diagonal step cuts a blocked corner; returns its cost.
fn route_cost(grid: &MapGrid, from: Cell, route: &[Cell]) -> u32 {
    let mut cost = 0;
    let mut at = from;
    for next in route {
        let (dx, dz) = (signed(next.0) - signed(at.0), signed(next.1) - signed(at.1));
        assert!(
            dx.abs() <= 1 && dz.abs() <= 1 && (dx, dz) != (0, 0),
            "{at:?} to {next:?}"
        );
        assert!(
            open_at(grid, signed(next.0), signed(next.1)),
            "{next:?} is open"
        );
        if dx != 0 && dz != 0 {
            assert!(
                open_at(grid, signed(at.0) + dx, signed(at.1)),
                "corner cut at {at:?}"
            );
            assert!(
                open_at(grid, signed(at.0), signed(at.1) + dz),
                "corner cut at {at:?}"
            );
        }
        cost += step_cost(at, *next);
        at = *next;
    }
    cost
}

/// The cheapest cost from `from` to `to` by repeated relaxation over every
/// cell, an algorithm unlike A*; `None` when unreachable.
fn dijkstra(grid: &MapGrid, from: Cell, to: Cell) -> Option<u32> {
    let (w, h) = (signed(grid.width), signed(grid.height));
    let mut best = vec![u32::MAX; (w * h) as usize];
    best[(signed(from.1) * w + signed(from.0)) as usize] = 0;
    loop {
        let mut changed = false;
        for z in 0..h {
            for x in 0..w {
                let here = best[(z * w + x) as usize];
                if here == u32::MAX {
                    continue;
                }
                for (dx, dz) in [
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                    (-1, 0),
                    (1, 0),
                    (-1, 1),
                    (0, 1),
                    (1, 1),
                ] {
                    let (nx, nz) = (x + dx, z + dz);
                    if !open_at(grid, nx, nz) {
                        continue;
                    }
                    if dx != 0 && dz != 0 && !(open_at(grid, x + dx, z) && open_at(grid, x, z + dz))
                    {
                        continue;
                    }
                    let cost = here + if dx != 0 && dz != 0 { 14 } else { 10 };
                    let slot = &mut best[(nz * w + nx) as usize];
                    if cost < *slot {
                        *slot = cost;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    let found = best[(signed(to.1) * w + signed(to.0)) as usize];
    (found != u32::MAX).then_some(found)
}

/// The planner of SIM-066 written out again, with the heap ordered by the
/// estimate ascending, the cost so far descending, and the tie-break on
/// the cell index ascending (`high` false) or descending (`high` true, the
/// control that differs from the clause only there).
fn astar(grid: &MapGrid, from: Cell, to: Cell, high: bool) -> Option<Vec<Cell>> {
    let (w, h) = (signed(grid.width), signed(grid.height));
    if !open_at(grid, signed(from.0), signed(from.1)) || !open_at(grid, signed(to.0), signed(to.1))
    {
        return None;
    }
    let index = |c: Cell| signed(c.1) * w + signed(c.0);
    let key = |i: i64| if high { -i } else { i };
    let heuristic = |c: Cell| {
        let (dx, dz) = (
            (signed(c.0) - signed(to.0)).abs(),
            (signed(c.1) - signed(to.1)).abs(),
        );
        u32::try_from(10 * dx.max(dz) + 4 * dx.min(dz)).expect("small")
    };
    let n = (w * h) as usize;
    let mut g = vec![u32::MAX; n];
    let mut parent: Vec<Option<i64>> = vec![None; n];
    let mut closed = vec![false; n];
    let mut heap = BinaryHeap::new();
    g[index(from) as usize] = 0;
    heap.push(Reverse((heuristic(from), u32::MAX, key(index(from)))));
    while let Some(Reverse((_, _, k))) = heap.pop() {
        let i = if high { -k } else { k };
        if closed[i as usize] {
            continue;
        }
        closed[i as usize] = true;
        if i == index(to) {
            break;
        }
        let (x, z) = (i % w, i / w);
        for dz in -1..=1_i64 {
            for dx in -1..=1_i64 {
                let (nx, nz) = (x + dx, z + dz);
                if (dx, dz) == (0, 0) || !open_at(grid, nx, nz) {
                    continue;
                }
                if dx != 0 && dz != 0 && !(open_at(grid, x + dx, z) && open_at(grid, x, z + dz)) {
                    continue;
                }
                let cost = if dx != 0 && dz != 0 { 14 } else { 10 };
                let ng = g[i as usize] + cost;
                let ni = nz * w + nx;
                if ng < g[ni as usize] {
                    g[ni as usize] = ng;
                    parent[ni as usize] = Some(i);
                    let cell = (u32::try_from(nx).expect("x"), u32::try_from(nz).expect("z"));
                    heap.push(Reverse((ng + heuristic(cell), u32::MAX - ng, key(ni))));
                }
            }
        }
    }
    let goal = index(to);
    if g[goal as usize] == u32::MAX {
        return None;
    }
    let mut route = Vec::new();
    let mut at = goal;
    while at != index(from) {
        route.push((
            u32::try_from(at % w).expect("x"),
            u32::try_from(at / w).expect("z"),
        ));
        at = parent[at as usize].expect("a parent on the way back");
    }
    route.reverse();
    Some(route)
}

// ---- SIM-066 ----------------------------------------------------------------

/// SIM-066: the planner returns a cheapest route (costs 10 and 14) that
/// excludes the start and ends at the goal, never cuts a blocked corner, and
/// is `None` for an unreachable or impassable goal.
#[test]
fn sim_planner_shortest_path_and_no_corner_cutting() {
    // An 8x8 wall at x = 3 with one gap at z = 3. The cheapest route from
    // (0, 0) to (7, 0) must enter the gap from (2, 3), because the diagonal
    // from (2, 2) cuts the blocked (3, 2): two diagonals and a step to
    // (2, 3) cost 38, the gap and (4, 3) cost 20, three diagonals to (7, 0)
    // cost 42, 100 in all.
    let wall = grid_of(8, 8, |x, z| x != 3 || z == 3);
    let route = plan_route(&wall, Layer::Land, (0, 0), (7, 0));
    let route = route.unwrap_or_default();
    assert_eq!(route.last(), Some(&(7, 0)));
    assert!(!route.contains(&(0, 0)), "the start is excluded");
    assert!(route.contains(&(3, 3)), "the route passes the gap");
    assert_eq!(route_cost(&wall, (0, 0), &route), 100);
    assert_eq!(dijkstra(&wall, (0, 0), (7, 0)), Some(100));

    // One blocked orthogonal refuses the diagonal: (0, 0) to (1, 1) with
    // (1, 0) blocked goes through (0, 1).
    let one = grid_of(3, 3, |x, z| (x, z) != (1, 0));
    assert_eq!(
        plan_route(&one, Layer::Land, (0, 0), (1, 1)),
        Some(vec![(0, 1), (1, 1)])
    );
    // Both blocked: no route at all.
    let both = grid_of(2, 2, |x, z| (x, z) == (0, 0) || (x, z) == (1, 1));
    assert_eq!(plan_route(&both, Layer::Land, (0, 0), (1, 1)), None);

    // Unreachable, impassable, off the grid, absent layer, and the same cell.
    let split = grid_of(8, 8, |x, _| x != 3);
    assert_eq!(plan_route(&split, Layer::Land, (0, 0), (7, 7)), None);
    assert_eq!(plan_route(&wall, Layer::Land, (0, 0), (3, 0)), None);
    assert_eq!(plan_route(&wall, Layer::Land, (3, 0), (0, 0)), None);
    assert_eq!(plan_route(&wall, Layer::Land, (9, 9), (0, 0)), None);
    assert_eq!(plan_route(&wall, Layer::Land, (0, 0), (9, 9)), None);
    assert_eq!(plan_route(&wall, Layer::Water, (0, 0), (1, 1)), None);
    assert_eq!(
        plan_route(&wall, Layer::Land, (2, 2), (2, 2)),
        Some(Vec::new())
    );
    // The air layer is open everywhere: straight over the wall.
    assert_eq!(
        plan_route(&wall, Layer::Air, (0, 0), (7, 0)).map(|r| r.len()),
        Some(7)
    );
}

// ---- SIM-067 ----------------------------------------------------------------

/// A seeded generator: a 64-bit linear congruential step, one value a call.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u32) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        u32::try_from((self.0 >> 33) % u64::from(bound)).expect("below the bound")
    }
}

/// The two corridors of a seed, as cells: from S = (sx, row) up (or down)
/// by d, across to gx, and back to G = (gx, row).
struct Corridors {
    start: Cell,
    goal: Cell,
    upper: Vec<Cell>,
    lower: Vec<Cell>,
}

fn corridors(rng: &mut Lcg) -> Corridors {
    let row = 7 + rng.next(2);
    let d = 1 + rng.next(5);
    let sx = 1 + rng.next(4);
    let gx = sx + 3 + rng.next(8);
    let side = |sign: i64| -> Vec<Cell> {
        let at = |z: i64| u32::try_from(i64::from(row) + sign * z).expect("in map");
        let mut cells: Vec<Cell> = (1..=i64::from(d)).map(|k| (sx, at(k))).collect();
        cells.extend((sx + 1..=gx).map(|x| (x, at(i64::from(d)))));
        cells.extend((0..i64::from(d)).rev().map(|k| (gx, at(k))));
        cells
    };
    Corridors {
        start: (sx, row),
        goal: (gx, row),
        upper: side(-1),
        lower: side(1),
    }
}

/// The 16x16 grid of a seed: every cell blocked except the corridors (both
/// in tie mode, the upper only otherwise), their two ends, and decoy cells
/// in rows 0 and 15, which no corridor touches.
fn seeded_grid(seed: u64, ties: bool) -> (MapGrid, Corridors) {
    let mut rng = Lcg(seed);
    let c = corridors(&mut rng);
    let decoys: Vec<bool> = (0..32).map(|_| rng.next(2) == 1).collect();
    let mut open: Vec<Cell> = vec![c.start, c.goal];
    open.extend(c.upper.iter().copied());
    if ties {
        open.extend(c.lower.iter().copied());
    }
    let grid = grid_of(16, 16, |x, z| {
        open.contains(&(x, z))
            || (z == 0 && decoys[x as usize])
            || (z == 15 && decoys[16 + x as usize])
    });
    (grid, c)
}

/// SIM-067: the planner is a pure function of grid, layer, start, and goal;
/// between equal-cost routes it takes the one that expands lower cell
/// indices first, and a control planner that prefers higher indices takes
/// the other.
#[test]
fn sim_planner_breaks_ties_by_cell_index() {
    // Hand-built: a 3x3 grid blocked at the centre. From (0, 1) to (2, 1)
    // the routes over the top row and over the bottom row both cost 40; the
    // top row has the lower indices and is the route the rule selects.
    let ring = grid_of(3, 3, |x, z| (x, z) != (1, 1));
    let top = vec![(0, 0), (1, 0), (2, 0), (2, 1)];
    let bottom = vec![(0, 2), (1, 2), (2, 2), (2, 1)];
    assert_eq!(
        plan_route(&ring, Layer::Land, (0, 1), (2, 1)),
        Some(top.clone())
    );
    assert_eq!(astar(&ring, (0, 1), (2, 1), true), Some(bottom));
    assert_eq!(dijkstra(&ring, (0, 1), (2, 1)), Some(40));

    // 64 seeds with two equal-cost corridors: the planner takes the upper
    // one (rows above, lower indices), twice over; the control takes the
    // lower one.
    for seed in 1..=64_u64 {
        let (grid, c) = seeded_grid(seed, true);
        let first = plan_route(&grid, Layer::Land, c.start, c.goal);
        let second = plan_route(&grid, Layer::Land, c.start, c.goal);
        assert_eq!(first, second, "seed {seed}: a pure function");
        assert_eq!(first.as_ref(), Some(&c.upper), "seed {seed}: ties by index");
        assert_eq!(
            route_cost(&grid, c.start, &c.upper),
            route_cost(&grid, c.start, &c.lower)
        );
        assert_eq!(
            dijkstra(&grid, c.start, c.goal),
            Some(route_cost(&grid, c.start, &c.upper))
        );
        let control = astar(&grid, c.start, c.goal, true);
        assert_eq!(control.as_ref(), Some(&c.lower), "seed {seed}: the control");
        assert_ne!(first, control, "seed {seed}: the control differs on ties");
        assert_eq!(astar(&grid, c.start, c.goal, false), first, "seed {seed}");
    }

    // 64 seeds with one corridor: the route is unique, so the planner, its
    // repeat, and the control all agree.
    for seed in 1..=64_u64 {
        let (grid, c) = seeded_grid(seed, false);
        let first = plan_route(&grid, Layer::Land, c.start, c.goal);
        assert_eq!(
            first,
            plan_route(&grid, Layer::Land, c.start, c.goal),
            "seed {seed}"
        );
        assert_eq!(
            first.as_ref(),
            Some(&c.upper),
            "seed {seed}: the only route"
        );
        assert_eq!(
            astar(&grid, c.start, c.goal, true),
            first,
            "seed {seed}: no tie, no difference"
        );
    }
}
