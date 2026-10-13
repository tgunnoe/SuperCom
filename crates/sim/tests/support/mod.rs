//! Synthetic worlds for the economy and build tests: no install, no file.
//! Every expected number in the tests is stated beside its assertion and
//! computed by integer arithmetic on raw fixed-point bits, never by calling
//! the helpers under test.

#![allow(
    dead_code,
    clippy::expect_used,
    clippy::panic,
    reason = "test support: not every test file uses every helper, and a missing fixture is a test failure"
)]

use std::collections::BTreeSet;
use std::sync::Arc;

use supercom_sim::{
    Army, BuildRestriction, Command, Digest, EntityId, Layer, LayerSet, MapGrid, Marker, Order,
    Pos, Q, QueueMode, State, Tick, UnitKind, World,
};

/// The integer `n` as a fixed-point number.
pub fn q(n: i64) -> Q {
    Q::from_bits(n << 32)
}

/// `num / den` in fixed point by integer division of raw bits, truncating
/// toward zero as the type's own division does.
pub fn frac(num: i64, den: i64) -> Q {
    Q::from_bits((num << 32) / den)
}

pub fn pos(x: f64, y: f64, z: f64) -> Pos {
    Pos { x, y, z }
}

pub fn cats(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

/// A kind with the given id and categories, buildable on land, health 100.
pub fn kind(id: &str, categories: &[&str]) -> UnitKind {
    UnitKind {
        id: id.to_owned(),
        categories: cats(categories),
        build_on: LayerSet::from_caps("Land").expect("a valid caps string"),
        health: q(100),
        max_health: q(100),
        ..UnitKind::default()
    }
}

/// A builder kind of `rate` that can build `buildable`.
pub fn builder(id: &str, rate: i64, buildable: &[&str]) -> UnitKind {
    let mut k = kind(id, &["MOBILE", "ENGINEER"]);
    k.build_rate = Some(q(rate));
    k.buildable = cats(buildable);
    k.storage_mass = Some(q(100_000));
    k.storage_energy = Some(q(1_000_000));
    k
}

/// A structure kind with the given cost and build time.
pub fn structure(id: &str, mass: i64, energy: i64, build_time: i64) -> UnitKind {
    let mut k = kind(id, &["STRUCTURE"]);
    k.build_cost_mass = q(mass);
    k.build_cost_energy = q(energy);
    k.build_time = Some(q(build_time));
    k
}

pub fn restricted(mut k: UnitKind, restriction: BuildRestriction) -> UnitKind {
    k.build_restriction = restriction;
    k
}

/// A flat `size` by `size` map, every cell land-passable.
pub fn flat_map(size: u32) -> MapGrid {
    let samples = vec![0_i16; ((size + 1) * (size + 1)) as usize];
    MapGrid::from_heightmap(size, size, &samples, 0.007_812_5).expect("a flat map")
}

pub fn world(units: Vec<UnitKind>, markers: Vec<Marker>, map: MapGrid) -> Arc<World> {
    let mut w = World::empty();
    w.units = units.into_iter().map(|k| (k.id.clone(), k)).collect();
    w.markers = markers;
    w.map = map;
    Arc::new(w)
}

/// A state in `world` with army 1 holding `mass` and `energy`, storage as
/// the kinds give it once units exist.
pub fn state(world: &Arc<World>, mass: Q, energy: Q) -> State {
    let mut s = State::with_world(Arc::clone(world), 1);
    s.add_army(Army(1), mass, energy, Q::ZERO, Q::ZERO);
    s
}

pub fn order_cmd(tick: u64, seq: u32, unit: EntityId, order: Order, mode: QueueMode) -> Command {
    Command {
        tick: Tick(tick),
        source: 0,
        seq,
        army: Army(1),
        units: vec![unit],
        order,
        mode,
    }
}

/// Step once with the commands stamped for the current tick.
pub fn step(s: &mut State, commands: &[Command]) -> Digest {
    s.step(commands).expect("a valid tick")
}

pub fn build_at(blueprint: &str, x: f64, z: f64) -> Order {
    Order::BuildMobile {
        blueprint: blueprint.to_owned(),
        at: pos(x, 0.0, z),
    }
}

pub fn land_layer() -> Layer {
    Layer::Land
}
