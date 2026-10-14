//! Guards on the choices of the planner that corridor grids cannot tell
//! apart (docs/specs/sim.md, SIM-066, SIM-067): over seeded random
//! obstacle fields the planner returns the route and the expansion count of
//! a planner written here, and the route's cost is the cheapest by a
//! relaxation search. A heap that ignores the cost so far, a zero or an
//! inadmissible estimate, and a different diagonal cost each change a route
//! or a count on some seed. All synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use supercom_sim::{Layer, MapGrid, Q, plan_route_expansions};

const SIZE: i64 = 16;

type Cell = (u32, u32);

fn grid(open: &[bool]) -> MapGrid {
    let mut passable = BTreeMap::new();
    passable.insert(Layer::Land, open.to_vec());
    MapGrid {
        width: SIZE as u32,
        height: SIZE as u32,
        heights: vec![Q::ZERO; ((SIZE + 1) * (SIZE + 1)) as usize],
        passable,
        water: None,
    }
}

/// A 64-bit linear congruential generator, one value a call.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }
}

fn is_open(open: &[bool], x: i64, z: i64) -> bool {
    (0..SIZE).contains(&x) && (0..SIZE).contains(&z) && open[(z * SIZE + x) as usize]
}

fn step_cost(dx: i64, dz: i64) -> u32 {
    if dx != 0 && dz != 0 { 14 } else { 10 }
}

/// Whether the step `(dx, dz)` from `(x, z)` is allowed: the target is open
/// and a diagonal cuts no blocked corner.
fn allowed(open: &[bool], x: i64, z: i64, dx: i64, dz: i64) -> bool {
    (dx, dz) != (0, 0)
        && is_open(open, x + dx, z + dz)
        && (dx == 0 || dz == 0 || (is_open(open, x + dx, z) && is_open(open, x, z + dz)))
}

/// The planner of SIM-066 and SIM-067 written out again: costs 10 and 14,
/// the estimate 10 times the larger axis distance plus 4 times the smaller,
/// the open set ordered by estimate ascending, cost so far descending, and
/// index ascending, neighbours in ascending index, a strictly smaller cost
/// replacing a parent. Returns the route and the cells expanded.
fn restated(open: &[bool], from: Cell, to: Cell) -> Option<(Vec<Cell>, usize)> {
    let index = |x: i64, z: i64| z * SIZE + x;
    let (fx, fz) = (i64::from(from.0), i64::from(from.1));
    let (tx, tz) = (i64::from(to.0), i64::from(to.1));
    if !is_open(open, fx, fz) || !is_open(open, tx, tz) {
        return None;
    }
    let h = |x: i64, z: i64| {
        let (dx, dz) = ((x - tx).abs(), (z - tz).abs());
        (10 * dx.max(dz) + 4 * dx.min(dz)) as u32
    };
    let n = (SIZE * SIZE) as usize;
    let mut g = vec![u32::MAX; n];
    let mut parent = vec![None::<i64>; n];
    let mut closed = vec![false; n];
    let mut heap = BinaryHeap::new();
    g[index(fx, fz) as usize] = 0;
    heap.push(Reverse((h(fx, fz), u32::MAX, index(fx, fz))));
    let mut expanded = 0;
    while let Some(Reverse((_, _, i))) = heap.pop() {
        if closed[i as usize] {
            continue;
        }
        closed[i as usize] = true;
        expanded += 1;
        if i == index(tx, tz) {
            break;
        }
        let (x, z) = (i % SIZE, i / SIZE);
        for dz in -1..=1_i64 {
            for dx in -1..=1_i64 {
                if !allowed(open, x, z, dx, dz) {
                    continue;
                }
                let ng = g[i as usize] + step_cost(dx, dz);
                let ni = index(x + dx, z + dz);
                if ng < g[ni as usize] {
                    g[ni as usize] = ng;
                    parent[ni as usize] = Some(i);
                    heap.push(Reverse((ng + h(x + dx, z + dz), u32::MAX - ng, ni)));
                }
            }
        }
    }
    let goal = index(tx, tz);
    if g[goal as usize] == u32::MAX {
        return None;
    }
    let mut route = Vec::new();
    let mut at = goal;
    while at != index(fx, fz) {
        route.push(((at % SIZE) as u32, (at / SIZE) as u32));
        at = parent[at as usize].expect("a parent on the way back");
    }
    route.reverse();
    Some((route, expanded))
}

/// The cheapest cost from `from` to `to` by repeated relaxation over every
/// cell, an algorithm unlike A*.
fn cheapest(open: &[bool], from: Cell, to: Cell) -> Option<u32> {
    let mut cost = vec![u32::MAX; (SIZE * SIZE) as usize];
    cost[(i64::from(from.1) * SIZE + i64::from(from.0)) as usize] = 0;
    loop {
        let mut changed = false;
        for z in 0..SIZE {
            for x in 0..SIZE {
                let here = cost[(z * SIZE + x) as usize];
                if here == u32::MAX {
                    continue;
                }
                for dz in -1..=1_i64 {
                    for dx in -1..=1_i64 {
                        if allowed(open, x, z, dx, dz) {
                            let slot = &mut cost[((z + dz) * SIZE + x + dx) as usize];
                            if here + step_cost(dx, dz) < *slot {
                                *slot = here + step_cost(dx, dz);
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    let c = cost[(i64::from(to.1) * SIZE + i64::from(to.0)) as usize];
    (c != u32::MAX).then_some(c)
}

fn route_cost(from: Cell, route: &[Cell]) -> u32 {
    let mut at = from;
    let mut total = 0;
    for c in route {
        total += step_cost(
            i64::from(c.0) - i64::from(at.0),
            i64::from(c.1) - i64::from(at.1),
        );
        at = *c;
    }
    total
}

/// 192 seeded obstacle fields of about two in ten cells: the planner's
/// route and expansion count equal the restated planner's, and the route
/// costs the relaxation search's minimum; at least a sixth of the seeds have
/// a route of more than ten cells, so the choices matter.
#[test]
fn sim_planner_matches_a_restated_planner_on_random_fields() {
    let (mut routed, mut long) = (0, 0);
    for seed in 1..=192_u64 {
        let mut rng = Lcg(seed);
        let open: Vec<bool> = (0..SIZE * SIZE).map(|_| rng.next(10) >= 2).collect();
        let pick = |rng: &mut Lcg| -> Cell {
            let i = rng.next((SIZE * SIZE) as u64) as i64;
            ((i % SIZE) as u32, (i / SIZE) as u32)
        };
        let (from, to) = (pick(&mut rng), pick(&mut rng));
        let g = grid(&open);
        let got = plan_route_expansions(&g, Layer::Land, from, to);
        let want = restated(&open, from, to);
        assert_eq!(got, want, "seed {seed}: {from:?} to {to:?}");
        if let Some((route, _)) = got {
            routed += 1;
            long += usize::from(route.len() > 10);
            assert_eq!(
                cheapest(&open, from, to),
                Some(route_cost(from, &route)),
                "seed {seed}: the route is a cheapest one"
            );
        } else if is_open(&open, i64::from(from.0), i64::from(from.1))
            && is_open(&open, i64::from(to.0), i64::from(to.1))
        {
            assert_eq!(cheapest(&open, from, to), None, "seed {seed}: unreachable");
        }
    }
    assert!(routed >= 96, "{routed} seeds have a route");
    assert!(long >= 32, "{long} seeds have a long route");
}
