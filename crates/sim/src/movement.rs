//! The movement phase (SIM-064 to SIM-070): layer and height assignment,
//! route planning and following by the integration rule, orders, and guard
//! following. Positions, speeds, and headings are binary64 under exact
//! operations; every sine, cosine, arc tangent, and square root is libm's
//! (DET-018 to DET-020).

use core::f64::consts::PI;
use std::sync::Arc;

use crate::build::{BUILD_RANGE, kind_of};
use crate::command::Order;
use crate::ids::{EntityId, Pos};
use crate::planner::plan_with;
use crate::state::{Entity, State};
use crate::world::{Layer, MapGrid, MotionType, SHALLOW, UnitKind, World};

/// One tick in seconds: a tenth, from SIM-001 and SIM-002 (SIM-069).
pub const TICK_SECONDS: f64 = 0.1;

/// How near a cell centre or a goal counts as reached, in world units,
/// compared as squares (SIM-065).
pub const ARRIVAL_RADIUS: f64 = 0.5;

/// How near a guard stays to its target, equal to the build range
/// (SIM-068).
pub const FOLLOW_RANGE: f64 = BUILD_RANGE;

/// The cell holding a position: the floor of x and z; `None` when either is
/// negative, not a number, or beyond `u32` (SIM-064).
#[must_use]
pub fn cell_of(pos: &Pos) -> Option<(u32, u32)> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the floor of a finite value checked to lie in 0..=u32::MAX is an exact integer in range"
    )]
    let floor = |v: f64| -> Option<u32> {
        let f = v.floor();
        (v.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&f)).then_some(f as u32)
    };
    Some((floor(pos.x)?, floor(pos.z)?))
}

/// The centre of a cell in world units: each coordinate plus a half
/// (SIM-064).
#[must_use]
pub fn cell_centre(cell: (u32, u32)) -> (f64, f64) {
    (f64::from(cell.0) + 0.5, f64::from(cell.1) + 0.5)
}

/// Whether a kind of `motion` can stand on cell `(x, z)` (SIM-064, SIM-070):
/// the cell is passable on one of the layers the motion type uses, a hover
/// unit crossing any water deeper than the land bound.
pub(crate) fn can_enter(map: &MapGrid, motion: MotionType, x: u32, z: u32) -> bool {
    let on = |layer: Layer| map.passable(layer, x, z);
    match motion {
        MotionType::None => false,
        MotionType::Air => on(Layer::Air),
        MotionType::Land => on(Layer::Land),
        MotionType::Water => on(Layer::Water),
        MotionType::SurfacingSub => on(Layer::Sub),
        MotionType::Amphibious => on(Layer::Land) || on(Layer::Seabed),
        MotionType::AmphibiousFloating => on(Layer::Land) || on(Layer::Water),
        MotionType::Hover => on(Layer::Land) || map.depth_at(x, z).is_some_and(|d| d > SHALLOW),
    }
}

/// The layer a kind of `motion` is on at `cell` (SIM-064, SIM-070).
fn layer_for(map: &MapGrid, motion: MotionType, cell: Option<(u32, u32)>) -> Layer {
    let Some((x, z)) = cell else {
        return Layer::Land;
    };
    let on = |layer: Layer| map.passable(layer, x, z);
    match motion {
        MotionType::Air => Layer::Air,
        MotionType::Water => Layer::Water,
        MotionType::SurfacingSub => Layer::Sub,
        MotionType::Amphibious if on(Layer::Seabed) && !on(Layer::Land) => Layer::Seabed,
        MotionType::AmphibiousFloating if on(Layer::Water) => Layer::Water,
        MotionType::Hover if map.depth_at(x, z).is_some_and(|d| d > SHALLOW) => Layer::Water,
        _ => Layer::Land,
    }
}

/// The height of an entity of `kind` on `layer` at `cell` (SIM-064,
/// SIM-070): the terrain, or the water surface for the water and sub layers,
/// lifted by the elevation of an air, hover, or surfacing-sub kind.
fn height_for(map: &MapGrid, kind: &UnitKind, layer: Layer, cell: (u32, u32)) -> f64 {
    let ground = map.height_at(cell.0, cell.1).to_num::<f64>();
    let level = match layer {
        Layer::Water | Layer::Sub => map.water.map_or(ground, |s| s.to_num::<f64>()),
        _ => ground,
    };
    let lift = match (kind.motion, layer) {
        (MotionType::Air | MotionType::Hover, _) | (MotionType::SurfacingSub, Layer::Sub) => {
            kind.elevation.unwrap_or(0.0)
        }
        _ => 0.0,
    };
    level + lift
}

/// A Stop at the head of an entity's queue: clear its work, route, and goal,
/// set its speed to 0, and pop it. The build phase and the movement phase
/// both call this, whichever sees the Stop first (SIM-068).
pub(crate) fn stop_head(e: &mut Entity) {
    e.work = None;
    e.route.clear();
    e.goal = None;
    e.speed = 0.0;
    e.orders.pop_front();
}

/// The dynamics a kind states; an absent field is the instant default
/// (SIM-065).
struct Dynamics {
    max_speed: f64,
    acceleration: Option<f64>,
    brake: Option<f64>,
    turn_rate: Option<f64>,
}

/// An angle wrapped into `(-PI, PI]` by whole turns (SIM-065).
fn wrap(angle: f64) -> f64 {
    let mut a = angle;
    while a > PI {
        a -= 2.0 * PI;
    }
    while a <= -PI {
        a += 2.0 * PI;
    }
    a
}

/// Whether the offset `(dx, dz)` is within [`ARRIVAL_RADIUS`].
fn within_arrival(dx: f64, dz: f64) -> bool {
    dx * dx + dz * dz <= ARRIVAL_RADIUS * ARRIVAL_RADIUS
}

/// One tick of SIM-065's rule for an entity with a goal; true when it ends
/// within the arrival radius of the goal.
fn advance(mover: &mut Entity, dynamics: &Dynamics, map: &MapGrid, motion: MotionType) -> bool {
    let Some(goal) = mover.goal else {
        return false;
    };
    let (px, pz) = (mover.pos.x, mover.pos.z);
    let (tx, tz) = mover
        .route
        .first()
        .map_or((goal.x, goal.z), |c| cell_centre(*c));
    let dx = tx - px;
    let dz = tz - pz;
    let desired = libm::atan2(dz, dx);
    let delta = wrap(desired - mover.heading);
    // Only a clamped step leaves the unit still turning. A completed turn
    // takes the desired heading itself: the sum heading + delta leaves a
    // residue near 5e-18 that must not read as turning (SIM-065).
    let (heading, turning) = match dynamics.turn_rate {
        Some(rate) => {
            let max_turn = rate * (PI / 180.0) * TICK_SECONDS;
            if delta.abs() > max_turn {
                (
                    wrap(mover.heading + delta.max(-max_turn).min(max_turn)),
                    true,
                )
            } else {
                (desired, false)
            }
        }
        None => (desired, false),
    };

    let mut remaining = 0.0_f64;
    let mut from = (px, pz);
    for to in mover
        .route
        .iter()
        .map(|c| cell_centre(*c))
        .chain(core::iter::once((goal.x, goal.z)))
    {
        let (ddx, ddz) = (to.0 - from.0, to.1 - from.1);
        remaining += libm::sqrt(ddx * ddx + ddz * ddz);
        from = to;
    }
    // A unit still turning sheds speed until it faces its target, else a
    // waypoint inside its turning circle is circled for ever (SIM-065).
    // The brake rate falls back to the acceleration, and to an instant stop
    // when the kind has neither, so no mover is left without a way to shed
    // speed (SIM-065).
    let speed = match dynamics.brake.or(dynamics.acceleration) {
        Some(rate) if turning || remaining <= mover.speed * mover.speed / (2.0 * rate) => {
            (mover.speed - rate * TICK_SECONDS).max(0.0)
        }
        None if turning || remaining <= 0.0 => 0.0,
        _ => match dynamics.acceleration {
            Some(rate) => dynamics.max_speed.min(mover.speed + rate * TICK_SECONDS),
            None => dynamics.max_speed,
        },
    };
    let stride = speed * TICK_SECONDS;
    let distance = libm::sqrt(dx * dx + dz * dz);
    let (nx, nz) = if mover.route.is_empty() && stride >= distance {
        (goal.x, goal.z)
    } else {
        (
            px + stride * libm::cos(heading),
            pz + stride * libm::sin(heading),
        )
    };
    let open = cell_of(&Pos {
        x: nx,
        y: mover.pos.y,
        z: nz,
    })
    .is_some_and(|(cx, cz)| can_enter(map, motion, cx, cz));
    mover.heading = heading;
    if open {
        mover.pos.x = nx;
        mover.pos.z = nz;
        mover.speed = speed;
    } else {
        // No clipping through a cliff: stay, route kept, speed 0.
        mover.speed = 0.0;
    }
    while let Some(c) = mover.route.first() {
        let (cx, cz) = cell_centre(*c);
        if !within_arrival(cx - mover.pos.x, cz - mover.pos.z) {
            break;
        }
        mover.route.remove(0);
    }
    within_arrival(goal.x - mover.pos.x, goal.z - mover.pos.z)
}

/// The cell a guard heads for: the target's own cell when the guard can
/// stand on it, else the first such cell of its eight neighbours in
/// ascending index (SIM-068).
fn guard_goal(map: &MapGrid, motion: MotionType, target: (u32, u32)) -> Option<(u32, u32)> {
    let around = |dz: i64, dx: i64| {
        let x = u32::try_from(i64::from(target.0).checked_add(dx)?).ok()?;
        let z = u32::try_from(i64::from(target.1).checked_add(dz)?).ok()?;
        Some((x, z))
    };
    let ok =
        |c: &(u32, u32)| c.0 < map.width && c.1 < map.height && can_enter(map, motion, c.0, c.1);
    // Ascending index is rows first, then columns; the target's own cell
    // comes before its neighbours.
    core::iter::once(Some(target))
        .chain([-1_i64, 0, 1].into_iter().flat_map(|dz| {
            [-1_i64, 0, 1]
                .into_iter()
                .filter(move |dx| (*dx, dz) != (0, 0))
                .map(move |dx| around(dz, dx))
        }))
        .flatten()
        .find(ok)
}

impl State {
    /// The movement phase: each complete mobile entity, in ascending id,
    /// applies the order at the head of its queue, then takes its layer and
    /// height from where it ends (SIM-064 to SIM-070).
    pub(crate) fn run_movement(&mut self) {
        let world = Arc::clone(self.world());
        let ids: Vec<EntityId> = self.entities.keys().copied().collect();
        for id in ids {
            self.move_entity(&world, id);
        }
    }

    fn move_entity(&mut self, world: &World, id: EntityId) {
        let Some(entity) = self.entities.get(&id).filter(|e| e.complete) else {
            return;
        };
        let Some(kind) = kind_of(world, &entity.blueprint) else {
            return;
        };
        let Some(max_speed) = kind.max_speed.filter(|_| kind.motion != MotionType::None) else {
            return;
        };
        let motion = kind.motion;
        let map = &world.map;
        let dynamics = Dynamics {
            max_speed,
            acceleration: kind.max_acceleration,
            brake: kind.max_brake,
            turn_rate: kind.turn_rate,
        };
        while let Some(head) = self
            .entities
            .get(&id)
            .and_then(|e| e.orders.front().cloned())
        {
            match head {
                Order::Stop => {
                    if let Some(e) = self.entities.get_mut(&id) {
                        stop_head(e);
                    }
                }
                Order::Move { to } => {
                    if !self.follow_move(map, motion, &dynamics, id, to) {
                        continue;
                    }
                    break;
                }
                Order::Guard { target } => {
                    let target_pos = self.entities.get(&target).map(|t| t.pos);
                    if self.follow_guard(map, motion, &dynamics, id, target_pos) {
                        break;
                    }
                }
                _ => {
                    if let Some(e) = self.entities.get_mut(&id) {
                        e.route.clear();
                        e.goal = None;
                        e.speed = 0.0;
                    }
                    break;
                }
            }
        }
        let Some(e) = self.entities.get_mut(&id) else {
            return;
        };
        if e.orders.is_empty() {
            e.route.clear();
            e.goal = None;
            e.speed = 0.0;
        }
        if let Some(cell) = cell_of(&e.pos) {
            e.layer = layer_for(map, motion, Some(cell));
            e.pos.y = height_for(map, kind, e.layer, cell);
        }
    }

    /// Plan the head Move if it is fresh, then advance along it; false when
    /// the order was dropped, so the caller looks at the next head.
    fn follow_move(
        &mut self,
        map: &MapGrid,
        motion: MotionType,
        dynamics: &Dynamics,
        id: EntityId,
        to: Pos,
    ) -> bool {
        let Some(e) = self.entities.get_mut(&id) else {
            return true;
        };
        if e.goal != Some(to) {
            let route = cell_of(&e.pos).zip(cell_of(&to)).and_then(|(from, goal)| {
                plan_with(map, |x, z| can_enter(map, motion, x, z), from, goal)
            });
            let Some(route) = route else {
                e.orders.pop_front();
                e.route.clear();
                e.goal = None;
                self.counters.commands_ignored = self.counters.commands_ignored.saturating_add(1);
                return false;
            };
            e.route = route;
            e.goal = Some(to);
        }
        if advance(e, dynamics, map, motion) {
            e.orders.pop_front();
            e.speed = 0.0;
            e.route.clear();
            e.goal = None;
            self.counters.moves_completed = self.counters.moves_completed.saturating_add(1);
        }
        true
    }

    /// Follow a guard's target; false when the Guard was dropped, so the
    /// caller looks at the next head.
    fn follow_guard(
        &mut self,
        map: &MapGrid,
        motion: MotionType,
        dynamics: &Dynamics,
        id: EntityId,
        target: Option<Pos>,
    ) -> bool {
        let Some(e) = self.entities.get_mut(&id) else {
            return true;
        };
        let Some(target) = target else {
            e.orders.pop_front();
            e.route.clear();
            e.goal = None;
            e.speed = 0.0;
            self.counters.commands_ignored = self.counters.commands_ignored.saturating_add(1);
            return false;
        };
        let (dx, dz) = (e.pos.x - target.x, e.pos.z - target.z);
        if dx * dx + dz * dz <= FOLLOW_RANGE * FOLLOW_RANGE {
            e.speed = 0.0;
            e.route.clear();
            e.goal = None;
            return true;
        }
        let goal = cell_of(&target).and_then(|c| guard_goal(map, motion, c));
        if goal.is_some() && goal != e.goal.as_ref().and_then(cell_of) {
            let route = cell_of(&e.pos).zip(goal).and_then(|(from, to)| {
                plan_with(map, |x, z| can_enter(map, motion, x, z), from, to)
            });
            if let Some((route, cell)) = route.zip(goal) {
                let (x, z) = cell_centre(cell);
                e.route = route;
                e.goal = Some(Pos { x, y: 0.0, z });
            } else {
                e.route.clear();
                e.goal = None;
            }
        }
        if e.goal.is_none() {
            e.speed = 0.0;
        } else if advance(e, dynamics, map, motion) {
            e.speed = 0.0;
            e.route.clear();
            e.goal = None;
        }
        true
    }
}
