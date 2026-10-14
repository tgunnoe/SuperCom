//! The route planner (SIM-066, SIM-067): A* over a passability predicate on
//! the eight-neighbour lattice, deterministic by construction.

use core::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::world::{Layer, MapGrid};

/// The cost of an orthogonal step, in the integer units of SIM-066.
const ORTHOGONAL: u32 = 10;

/// The cost of a diagonal step: ten times the square root of two, rounded.
const DIAGONAL: u32 = 14;

/// The cheapest route from `from` to `to` on `layer`, excluding the start
/// cell and ending at the goal cell; `None` when no route exists, the start
/// or the goal is impassable or off the grid (SIM-066), and empty when they
/// are one cell.
#[must_use]
pub fn plan_route(
    grid: &MapGrid,
    layer: Layer,
    from: (u32, u32),
    to: (u32, u32),
) -> Option<Vec<(u32, u32)>> {
    plan_with(grid, |x, z| grid.passable(layer, x, z), from, to)
}

/// [`plan_route`] over any passability predicate of the grid's cells, so a
/// kind that moves on two layers plans over their union (SIM-070). `open`
/// is asked only about cells inside the grid.
pub(crate) fn plan_with(
    grid: &MapGrid,
    open: impl Fn(u32, u32) -> bool,
    from: (u32, u32),
    to: (u32, u32),
) -> Option<Vec<(u32, u32)>> {
    let inside = |(x, z): (u32, u32)| x < grid.width && z < grid.height && open(x, z);
    if !inside(from) || !inside(to) {
        return None;
    }
    let width = usize::try_from(grid.width).ok()?;
    let cells = width.checked_mul(usize::try_from(grid.height).ok()?)?;
    let index = |(x, z): (u32, u32)| -> Option<usize> {
        usize::try_from(z)
            .ok()?
            .checked_mul(width)?
            .checked_add(usize::try_from(x).ok()?)
    };
    let (start, goal) = (index(from)?, index(to)?);
    let estimate = |(x, z): (u32, u32)| {
        let (dx, dz) = (x.abs_diff(to.0), z.abs_diff(to.1));
        ORTHOGONAL
            .saturating_mul(dx.max(dz))
            .saturating_add(4_u32.saturating_mul(dx.min(dz)))
    };
    let mut cost = vec![u32::MAX; cells];
    let mut parent: Vec<Option<usize>> = vec![None; cells];
    let mut closed = vec![false; cells];
    // Ordered by the estimate, the cost so far, then the cell index, each
    // ascending (SIM-067).
    let mut heap = BinaryHeap::new();
    *cost.get_mut(start)? = 0;
    heap.push(Reverse((estimate(from), 0_u32, start)));
    while let Some(Reverse((_, _, cell))) = heap.pop() {
        if closed.get(cell).copied().unwrap_or(true) {
            continue;
        }
        *closed.get_mut(cell)? = true;
        if cell == goal {
            break;
        }
        let here = (
            u32::try_from(cell % width).ok()?,
            u32::try_from(cell / width).ok()?,
        );
        let so_far = *cost.get(cell)?;
        // Neighbours in ascending cell index: rows first, then columns.
        for dz in [-1_i64, 0, 1] {
            for dx in [-1_i64, 0, 1] {
                let Some(next) = step(here, dx, dz).filter(|c| inside(*c)) else {
                    continue;
                };
                let diagonal = dx != 0 && dz != 0;
                if diagonal {
                    // Both orthogonal cells the step cuts must be open.
                    let open_corner = |c: Option<(u32, u32)>| c.is_some_and(inside);
                    if !open_corner(step(here, dx, 0)) || !open_corner(step(here, 0, dz)) {
                        continue;
                    }
                }
                let through = so_far.saturating_add(if diagonal { DIAGONAL } else { ORTHOGONAL });
                let slot = index(next)?;
                if through < *cost.get(slot)? {
                    *cost.get_mut(slot)? = through;
                    *parent.get_mut(slot)? = Some(cell);
                    heap.push(Reverse((
                        through.saturating_add(estimate(next)),
                        through,
                        slot,
                    )));
                }
            }
        }
    }
    if *cost.get(goal)? == u32::MAX {
        return None;
    }
    let mut route = Vec::new();
    let mut at = goal;
    while at != start {
        route.push((
            u32::try_from(at % width).ok()?,
            u32::try_from(at / width).ok()?,
        ));
        at = (*parent.get(at)?)?;
    }
    route.reverse();
    Some(route)
}

/// The cell offset from `cell` by `(dx, dz)`, `None` below zero or beyond
/// `u32`; `(0, 0)` is no step at all.
fn step(cell: (u32, u32), dx: i64, dz: i64) -> Option<(u32, u32)> {
    if (dx, dz) == (0, 0) {
        return None;
    }
    let x = i64::from(cell.0).checked_add(dx)?;
    let z = i64::from(cell.1).checked_add(dz)?;
    Some((u32::try_from(x).ok()?, u32::try_from(z).ok()?))
}
