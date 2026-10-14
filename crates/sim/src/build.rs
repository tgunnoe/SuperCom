//! The build phase (SIM-056, SIM-059 to SIM-061): consumption rates,
//! progress and assist, factory queues, and structure placement.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::command::Order;
use crate::economy::per_tick;
use crate::ids::{EntityId, Pos, Q};
use crate::movement::stop_head;
use crate::state::{Entity, State};
use crate::world::{BuildRestriction, Layer, MarkerKind, UnitKind, World};

/// The floor of a build time before the builder's rate divides it: raw bits
/// 429496730, the nearest [`Q`] to a tenth of a second (SIM-050).
pub const BUILD_TIME_FLOOR: Q = Q::from_bits(429_496_730);

/// How far a builder reaches, in world units from the nearest edge of the
/// site's footprint (SIM-061). Positions and distances are `f64` geometry
/// (ADR-0006); everything economic is [`Q`].
pub const BUILD_RANGE: f64 = 5.0;

/// What building one kind costs per second at one builder's rate (SIM-056).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildRates {
    pub seconds: Q,
    pub mass_per_second: Q,
    pub energy_per_second: Q,
}

/// The rates of building `kind` at `builder_rate`; `None` when the kind has
/// no build time or the rate is not positive (SIM-056).
#[must_use]
pub fn build_rates(kind: &UnitKind, builder_rate: Q) -> Option<BuildRates> {
    let time = kind.build_time?;
    if builder_rate <= Q::ZERO {
        return None;
    }
    let seconds = time.max(BUILD_TIME_FLOOR).saturating_div(builder_rate);
    if seconds <= Q::ZERO {
        return None;
    }
    Some(BuildRates {
        seconds,
        mass_per_second: kind.build_cost_mass.max(Q::ONE).saturating_div(seconds),
        energy_per_second: kind.build_cost_energy.max(Q::ONE).saturating_div(seconds),
    })
}

/// The cells a kind's footprint covers, width by depth; a missing block or
/// a zero dimension reads as 1 (SIM-061).
#[must_use]
pub fn footprint_of(kind: &UnitKind) -> (u32, u32) {
    kind.footprint.map_or((1, 1), |(w, d)| (w.max(1), d.max(1)))
}

/// The kind of an entity's blueprint, which is stored in lower case.
pub(crate) fn kind_of<'w>(world: &'w World, blueprint: &str) -> Option<&'w UnitKind> {
    world.units.get(blueprint)
}

/// The total work a kind takes, in build-rate-seconds: its build time,
/// floored (SIM-056). Completion and progress both read it.
#[must_use]
pub fn total_work(kind: &UnitKind) -> Q {
    kind.build_time
        .map_or(BUILD_TIME_FLOOR, |t| t.max(BUILD_TIME_FLOOR))
}

/// An axis-aligned block of map cells: `x0 .. x0 + w` by `z0 .. z0 + d`.
#[derive(Clone, Copy)]
struct Rect {
    x0: i64,
    z0: i64,
    w: i64,
    d: i64,
}

/// The integer floor of a coordinate, `None` when it is not a number or
/// does not fit.
fn floor_cell(v: f64) -> Option<i64> {
    if !v.is_finite() {
        return None;
    }
    Q::checked_from_num(v.floor()).map(|q| q.to_num::<i64>())
}

impl Rect {
    /// The block `kind` covers centred on `at`; a mobile kind covers the one
    /// cell of its position (SIM-061).
    fn of(kind: Option<&UnitKind>, at: &Pos) -> Option<Rect> {
        let mobile = kind.is_none_or(|k| k.categories.contains("MOBILE"));
        let (w, d) = match kind {
            Some(k) if !mobile => footprint_of(k),
            _ => (1, 1),
        };
        let (x0, z0) = if mobile {
            (floor_cell(at.x)?, floor_cell(at.z)?)
        } else {
            (
                floor_cell(at.x - f64::from(w) / 2.0)?,
                floor_cell(at.z - f64::from(d) / 2.0)?,
            )
        };
        Some(Rect {
            x0,
            z0,
            w: i64::from(w),
            d: i64::from(d),
        })
    }

    fn overlaps(self, other: Rect) -> bool {
        self.x0 < other.x0 + other.w
            && other.x0 < self.x0 + self.w
            && self.z0 < other.z0 + other.d
            && other.z0 < self.z0 + self.d
    }

    /// Whether `pos` is within [`BUILD_RANGE`] of the block's nearest point,
    /// compared as squares so no root is taken.
    fn reached_from(self, pos: &Pos) -> bool {
        let gap = |p: f64, lo: i64, extent: i64| -> f64 {
            // Cell coordinates are small integers, exact in binary64.
            let lo = Q::saturating_from_num(lo).to_num::<f64>();
            let hi = lo + Q::saturating_from_num(extent).to_num::<f64>();
            (lo - p).max(p - hi).max(0.0)
        };
        let dx = gap(pos.x, self.x0, self.w);
        let dz = gap(pos.z, self.z0, self.d);
        dx * dx + dz * dz <= BUILD_RANGE * BUILD_RANGE
    }
}

/// How a build order at the head of a queue fares this tick.
enum Start {
    /// Spawn this kind under construction at this position.
    Spawn(String, Pos),
    Wait,
    Drop,
}

impl State {
    /// The build phase: stops, work on units under construction, the starts
    /// of queued builds, then assists (SIM-059 to SIM-061).
    pub(crate) fn run_build(&mut self) {
        let world = Arc::clone(self.world());
        self.clear_dead_work();
        self.settle_stops(&world);
        let prior: BTreeMap<EntityId, EntityId> = self
            .entities
            .iter()
            .filter_map(|(id, e)| e.work.map(|t| (*id, t)))
            .collect();
        self.do_work(&world, &prior);
        self.start_builds(&world);
        self.assign_assists(&world);
    }

    /// A work item naming an entity that is gone or finished explains
    /// nothing; clearing it lets the head order be tried again.
    fn clear_dead_work(&mut self) {
        let live: BTreeSet<EntityId> = self
            .entities
            .iter()
            .filter(|(_, e)| !e.complete)
            .map(|(id, _)| *id)
            .collect();
        for e in self.entities.values_mut() {
            if e.work.is_some_and(|t| !live.contains(&t)) {
                e.work = None;
            }
        }
    }

    /// A Stop at the head of a builder's queue is handled by the shared Stop
    /// helper, which the movement phase also calls (SIM-068). It checks
    /// nothing else: work is set only under a build or Guard head, a
    /// replacing command clears it, and completion clears it before popping.
    fn settle_stops(&mut self, world: &World) {
        let ids: Vec<EntityId> = self.entities.keys().copied().collect();
        for id in ids {
            let building = self.entities.get(&id).is_some_and(|e| {
                e.complete && kind_of(world, &e.blueprint).is_some_and(|k| k.build_rate.is_some())
            });
            let Some(e) = self.entities.get_mut(&id) else {
                continue;
            };
            while building && matches!(e.orders.front(), Some(Order::Stop)) {
                stop_head(e);
            }
        }
    }

    /// Add this tick's work to every unit under construction whose
    /// contributors held their work item before the economy phase.
    fn do_work(&mut self, world: &World, prior: &BTreeMap<EntityId, EntityId>) {
        let targets: BTreeSet<EntityId> = prior.values().copied().collect();
        for target in targets {
            let Some(site) = self.entities.get(&target).filter(|t| !t.complete) else {
                continue;
            };
            let Some(kind) = kind_of(world, &site.blueprint) else {
                continue;
            };
            let Some(rect) = Rect::of(Some(kind), &site.pos) else {
                continue;
            };
            let mut contributors = Vec::new();
            let mut gained = Q::ZERO;
            for (who, _) in prior.iter().filter(|(_, t)| **t == target) {
                let Some(c) = self.entities.get(who) else {
                    continue;
                };
                let rate = kind_of(world, &c.blueprint)
                    .and_then(|k| k.build_rate)
                    .filter(|_| c.complete);
                if let Some(rate) = rate.filter(|_| rect.reached_from(&c.pos)) {
                    gained = gained.saturating_add(c.fraction.saturating_mul(per_tick(rate)));
                    contributors.push(*who);
                }
            }
            let total = total_work(kind);
            let Some(site) = self.entities.get_mut(&target) else {
                continue;
            };
            site.built = site.built.saturating_add(gained);
            if site.built >= total {
                site.built = total;
                site.complete = true;
                site.health = site.max_health;
                self.counters.builds_completed = self.counters.builds_completed.saturating_add(1);
                for who in prior.iter().filter(|(_, t)| **t == target).map(|(w, _)| w) {
                    if let Some(c) = self.entities.get_mut(who) {
                        c.work = None;
                        if matches!(
                            c.orders.front(),
                            Some(Order::BuildMobile { .. } | Order::BuildFactory { .. })
                        ) {
                            c.orders.pop_front();
                        }
                    }
                }
            } else {
                site.health = site.max_health.saturating_mul(site.progress(kind));
            }
        }
    }

    /// Start the build at the head of each idle builder's queue, dropping
    /// and counting orders that fail a check (SIM-061).
    fn start_builds(&mut self, world: &World) {
        let ids: Vec<EntityId> = self.entities.keys().copied().collect();
        for id in ids {
            while let Some(e) = self
                .entities
                .get(&id)
                .filter(|e| e.complete && e.work.is_none())
            {
                let army = e.army;
                let outcome = match e.orders.front() {
                    Some(Order::BuildMobile { blueprint, at }) => {
                        self.try_structure(world, e, blueprint, at)
                    }
                    Some(Order::BuildFactory { blueprint }) => {
                        Self::try_factory(world, e, blueprint)
                    }
                    _ => break,
                };
                match outcome {
                    Start::Spawn(key, pos) => {
                        let site = self.spawn_under_construction(army, &key, pos);
                        if let Some(b) = self.entities.get_mut(&id) {
                            b.work = site;
                        }
                        break;
                    }
                    Start::Wait => break,
                    Start::Drop => {
                        if let Some(b) = self.entities.get_mut(&id) {
                            b.orders.pop_front();
                        }
                        self.counters.commands_ignored =
                            self.counters.commands_ignored.saturating_add(1);
                    }
                }
            }
        }
    }

    /// The kind checks every build order shares: the builder can build the
    /// kind, the kind has a build time, and the builder has a rate.
    fn buildable<'w>(world: &'w World, builder: &Entity, blueprint: &str) -> Option<&'w UnitKind> {
        let key = blueprint.to_ascii_lowercase();
        let by = kind_of(world, &builder.blueprint)?;
        let target = world.units.get(&key)?;
        (by.buildable.contains(&key)
            && target.build_time.is_some()
            && by.build_rate.is_some_and(|r| r > Q::ZERO))
        .then_some(target)
    }

    fn try_factory(world: &World, builder: &Entity, blueprint: &str) -> Start {
        let Some(target) = Self::buildable(world, builder, blueprint) else {
            return Start::Drop;
        };
        if !target.categories.contains("MOBILE") {
            return Start::Drop;
        }
        Start::Spawn(target.id.clone(), builder.pos)
    }

    fn try_structure(&self, world: &World, builder: &Entity, blueprint: &str, at: &Pos) -> Start {
        let Some(target) = Self::buildable(world, builder, blueprint) else {
            return Start::Drop;
        };
        if !target.categories.contains("STRUCTURE") || !target.build_on.contains(Layer::Land) {
            return Start::Drop;
        }
        let Some(rect) = Rect::of(Some(target), at) else {
            return Start::Drop;
        };
        let Some(y) = Self::site_height(world, rect, at) else {
            return Start::Drop;
        };
        let site = Pos {
            x: at.x,
            y,
            z: at.z,
        };
        if !self.deposit_free(world, target, &site) || self.occupied(world, rect) {
            return Start::Drop;
        }
        if !rect.reached_from(&builder.pos) {
            return Start::Wait;
        }
        Start::Spawn(target.id.clone(), site)
    }

    /// The terrain height at the site, `None` unless every cell of `rect`
    /// lies inside the map and is land-passable (SIM-048).
    fn site_height(world: &World, rect: Rect, at: &Pos) -> Option<f64> {
        for dz in 0..rect.d {
            for dx in 0..rect.w {
                let x = u32::try_from(rect.x0 + dx).ok()?;
                let z = u32::try_from(rect.z0 + dz).ok()?;
                if !world.map.passable(Layer::Land, x, z) {
                    return None;
                }
            }
        }
        let x = u32::try_from(floor_cell(at.x)?).ok()?;
        let z = u32::try_from(floor_cell(at.z)?).ok()?;
        Some(world.map.height_at(x, z).to_num::<f64>())
    }

    /// Whether any entity's footprint overlaps `rect`.
    fn occupied(&self, world: &World, rect: Rect) -> bool {
        self.entities.values().any(|e| {
            let kind = kind_of(world, &e.blueprint);
            Rect::of(kind, &e.pos).is_some_and(|r| r.overlaps(rect))
        })
    }

    /// The marker rule of SIM-041 and SIM-042 for `target` at `site`: a kind
    /// without a restriction is always free; otherwise a marker of the
    /// matching kind must lie within 1 unit on each axis, exclusive, and no
    /// entity of a kind with the same restriction may hold it.
    fn deposit_free(&self, world: &World, target: &UnitKind, site: &Pos) -> bool {
        let wanted = match target.build_restriction {
            BuildRestriction::None => return true,
            BuildRestriction::OnMassDeposit => MarkerKind::Mass,
            BuildRestriction::OnHydrocarbonDeposit => MarkerKind::Hydrocarbon,
        };
        let near = |a: &Pos, b: &Pos| {
            (a.x - b.x).abs() < 1.0 && (a.y - b.y).abs() < 1.0 && (a.z - b.z).abs() < 1.0
        };
        world
            .markers
            .iter()
            .filter(|m| m.kind == wanted && near(&m.pos, site))
            .any(|m| {
                !self.entities.values().any(|e| {
                    kind_of(world, &e.blueprint)
                        .is_some_and(|k| k.build_restriction == target.build_restriction)
                        && near(&m.pos, &e.pos)
                })
            })
    }

    /// Point each guarding entity at the unit under construction it assists:
    /// the target itself or its builder's work item, when within range
    /// (SIM-059).
    fn assign_assists(&mut self, world: &World) {
        let ids: Vec<EntityId> = self.entities.keys().copied().collect();
        for id in ids {
            let Some(e) = self.entities.get(&id).filter(|e| e.complete) else {
                continue;
            };
            let Some(Order::Guard { target }) = e.orders.front() else {
                continue;
            };
            let under_construction = |t: EntityId| {
                self.entities
                    .get(&t)
                    .filter(|s| !s.complete)
                    .map(|s| (t, s))
            };
            let site = under_construction(*target).or_else(|| {
                self.entities
                    .get(target)
                    .and_then(|b| b.work)
                    .and_then(under_construction)
            });
            let helps = kind_of(world, &e.blueprint).is_some_and(|k| k.build_rate.is_some());
            let work = site
                .filter(|(t, _)| *t != id && helps)
                .filter(|(_, s)| {
                    Rect::of(kind_of(world, &s.blueprint), &s.pos)
                        .is_some_and(|r| r.reached_from(&e.pos))
                })
                .map(|(t, _)| t);
            if let Some(e) = self.entities.get_mut(&id) {
                e.work = work;
            }
        }
    }
}
