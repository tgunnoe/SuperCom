//! The route planner (SIM-066, SIM-067): A* over a layer's passability grid.

use crate::world::{Layer, MapGrid};

/// The cheapest route from `from` to `to` on `layer`, excluding the start
/// cell and ending at the goal cell; `None` when no route exists (SIM-066).
#[must_use]
pub fn plan_route(
    _grid: &MapGrid,
    _layer: Layer,
    _from: (u32, u32),
    _to: (u32, u32),
) -> Option<Vec<(u32, u32)>> {
    None
}
