//! The cost of a search under the heap order of docs/specs/sim.md,
//! SIM-067: among equal estimates the cell with the larger cost so far is
//! expanded first, so an open field is crossed along one route instead of
//! being swept across its whole band of equal-cost routes. The expected
//! count is the one a planner written here under the same order reaches.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use supercom_sim::{Layer, MapGrid, Q, plan_route_expansions};

fn open_field(size: u32) -> MapGrid {
    let cells = (size * size) as usize;
    let mut passable = BTreeMap::new();
    passable.insert(Layer::Land, vec![true; cells]);
    MapGrid {
        width: size,
        height: size,
        heights: vec![Q::ZERO; ((size + 1) * (size + 1)) as usize],
        passable,
        water: None,
    }
}

/// A* over an open `size` field from the corner to the far edge's middle,
/// costs 10 and 14, the estimate 10 times the larger axis distance plus 4
/// times the smaller, the open set ordered by estimate ascending, cost so
/// far descending, and index ascending; returns the cells expanded.
fn expansions(size: i64) -> usize {
    let (gx, gz) = (size - 1, size / 2);
    let goal = gz * size + gx;
    let h = |i: i64| {
        let (dx, dz) = ((gx - i % size).abs(), (gz - i / size).abs());
        10 * dx.max(dz) + 4 * dx.min(dz)
    };
    let mut g = vec![i64::MAX; (size * size) as usize];
    let mut closed = vec![false; (size * size) as usize];
    let mut heap = BinaryHeap::new();
    g[0] = 0;
    heap.push(Reverse((h(0), Reverse(0_i64), 0_i64)));
    let mut count = 0;
    while let Some(Reverse((_, _, i))) = heap.pop() {
        if closed[i as usize] {
            continue;
        }
        closed[i as usize] = true;
        count += 1;
        if i == goal {
            break;
        }
        let (x, z) = (i % size, i / size);
        for dz in -1..=1_i64 {
            for dx in -1..=1_i64 {
                let (nx, nz) = (x + dx, z + dz);
                if (dx, dz) == (0, 0) || nx < 0 || nz < 0 || nx >= size || nz >= size {
                    continue;
                }
                let ng = g[i as usize] + if dx != 0 && dz != 0 { 14 } else { 10 };
                let ni = nz * size + nx;
                if ng < g[ni as usize] {
                    g[ni as usize] = ng;
                    heap.push(Reverse((ng + h(ni), Reverse(ng), ni)));
                }
            }
        }
    }
    count
}

/// A plan across an open 32 by 32 field from a corner to the middle of the
/// far edge, where many routes cost the same, expands the cells the restated
/// order expands, and no more than twice the route's cells: under the
/// ascending cost order it expands the whole band of equal-cost routes. (A
/// corner-to-corner plan has one cheapest route, the diagonal, and no ties.)
#[test]
fn sim_planner_expands_deeper_cells_first_on_ties() {
    let want = expansions(32);
    assert!(want <= 64, "the restated order expands {want} cells");
    let grid = open_field(32);
    let (route, got) =
        plan_route_expansions(&grid, Layer::Land, (0, 0), (31, 16)).expect("a route");
    assert_eq!(route.len(), 31, "31 steps");
    assert_eq!(got, want, "expansions under the cost-descending order");
}
