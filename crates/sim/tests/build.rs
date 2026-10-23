//! Acceptance tests of the build rules (docs/specs/sim.md, SIM-041, SIM-042,
//! SIM-056, SIM-059 to SIM-061). All synthetic. Fixed-point values are
//! stated as raw bits computed by the integer arithmetic shown beside them.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::sync::Arc;

use supercom_sim::{
    Army, BuildRates, BuildRestriction, Entity, EntityId, Layer, Marker, MarkerKind, Order, Q,
    QueueMode, State, UnitKind, World, build_rates, footprint_of,
};
use support::{
    build_at, builder, flat_map, kind, order_cmd, pos, q, restricted, state, step, structure, world,
};

fn entity(s: &State, id: EntityId) -> &Entity {
    s.entities.get(&id).expect("the entity exists")
}

fn army(s: &State) -> &supercom_sim::ArmyState {
    s.armies.get(&Army(1)).expect("army 1 exists")
}

/// A state rich enough that no build stalls.
fn rich(w: &Arc<World>) -> State {
    state(w, q(1_000_000), q(1_000_000))
}

/// SIM-056: the installed formula with every modifier 0 and every
/// adjustment 1.
#[test]
fn sim_build_consumption_rates_from_costs() {
    let mut k = kind("k", &["STRUCTURE"]);
    k.build_time = Some(q(60));
    k.build_cost_mass = q(36);
    k.build_cost_energy = q(360);
    // 60 / 10 = 6 seconds; 36 / 6 = 6 and 360 / 6 = 60 per second.
    assert_eq!(
        build_rates(&k, q(10)),
        Some(BuildRates {
            seconds: q(6),
            mass_per_second: q(6),
            energy_per_second: q(60),
        })
    );

    // A build time of 0.05 (raw (1 << 32) / 20 = 214748364) floors at the
    // constant 429496730; / 10 is exactly 42949673 in raw bits.
    k.build_time = Some(Q::from_bits((1 << 32) / 20));
    let floored = build_rates(&k, q(10)).expect("a floored build time still builds");
    assert_eq!(floored.seconds.to_bits(), 42_949_673);

    // A cost of 0 floors at 1: 1 / 6 is raw (1 << 64) / (6 << 32) =
    // (1 << 32) / 6 = 715827882.
    k.build_time = Some(q(60));
    k.build_cost_mass = Q::ZERO;
    k.build_cost_energy = Q::ZERO;
    let cheap = build_rates(&k, q(10)).expect("a free kind still builds");
    assert_eq!(cheap.mass_per_second.to_bits(), 715_827_882);
    assert_eq!(cheap.energy_per_second.to_bits(), 715_827_882);

    // No build time, or a rate that is not positive, builds nothing.
    k.build_time = None;
    assert_eq!(build_rates(&k, q(10)), None);
    k.build_time = Some(q(60));
    assert_eq!(build_rates(&k, Q::ZERO), None);
    assert_eq!(build_rates(&k, q(-5)), None);
}

/// A world with a builder of rate 10 that builds `gen` (build time 60, cost
/// 60 mass and 600 energy: 1 and 10 per tick at rate 10), and a helper of
/// rate 5 that builds nothing.
fn plant_world() -> Arc<World> {
    let mut plant = structure("gen", 60, 600, 60);
    plant.footprint = Some((3, 3));
    world(
        vec![
            builder("worker", 10, &["gen"]),
            builder("helper", 5, &[]),
            plant,
        ],
        Vec::new(),
        flat_map(16),
    )
}

/// Spawn the worker (id 1) and the helper (id 2) within range of a site at
/// (8.5, 8.5), and command the worker to build there at tick 0.
fn start_plant(helper_at: (f64, f64)) -> (State, Vec<supercom_sim::Command>) {
    let w = plant_world();
    let mut s = rich(&w);
    let b = s.spawn(Army(1), "worker", pos(8.5, 0.0, 4.5), q(100));
    let h = s.spawn(
        Army(1),
        "helper",
        pos(helper_at.0, 0.0, helper_at.1),
        q(100),
    );
    let cmds = vec![
        order_cmd(0, 0, b, build_at("gen", 8.5, 8.5), QueueMode::Replace),
        order_cmd(0, 1, h, Order::Guard { target: b }, QueueMode::Replace),
    ];
    (s, cmds)
}

/// Run `ticks` ticks from tick 0 with `cmds` at tick 0.
fn run_plant(s: &mut State, cmds: &[supercom_sim::Command], ticks: u64) {
    for t in 0..ticks {
        step(s, if t == 0 { cmds } else { &[] });
    }
}

/// SIM-059: progress is the sum of the contributors' rates; health tracks
/// progress; a build commanded at tick t completes in the build phase of
/// t + 60 at rate 10, t + 40 with a rate-5 assister in range.
#[test]
fn sim_build_progress_and_assist_sum_rates() {
    // Alone: sixty increments of per_tick(10) = (10 << 32) / 10 = 1.
    let (mut s, cmds) = start_plant((50.0, 50.0));
    // The first item spawns in the build phase of tick 0 with no work.
    run_plant(&mut s, &cmds, 1);
    let site = EntityId(3);
    assert_eq!(entity(&s, site).built, Q::ZERO);
    assert!(!entity(&s, site).complete);
    // After the build phase of tick 30, thirty work ticks are done: built is
    // 30 and health is max health times 30 / 60, here 100 * 0.5 = 50.
    for _ in 1..=30 {
        step(&mut s, &[]);
    }
    assert_eq!(entity(&s, site).built, q(30));
    assert_eq!(entity(&s, site).health, q(50));
    for _ in 31..=59 {
        step(&mut s, &[]);
    }
    assert!(!entity(&s, site).complete, "not complete after tick 59");
    assert_eq!(s.counters.builds_completed, 0);
    step(&mut s, &[]); // tick 60
    let done = entity(&s, site);
    assert!(done.complete, "complete in the build phase of tick 60");
    assert_eq!(done.health, done.max_health);
    assert_eq!(s.counters.builds_completed, 1);
    assert!(entity(&s, EntityId(1)).orders.is_empty());
    assert_eq!(entity(&s, EntityId(1)).work, None);

    // With a rate-5 assister in range: 1 + 0.5 per tick, 60 / 1.5 = 40.
    let (mut s, cmds) = start_plant((6.5, 4.5));
    run_plant(&mut s, &cmds, 40);
    assert!(!entity(&s, site).complete, "not complete after tick 39");
    step(&mut s, &[]); // tick 40
    assert!(
        entity(&s, site).complete,
        "complete in the build phase of tick 40"
    );
    assert_eq!(entity(&s, EntityId(2)).work, None);

    // Out of range (more than 5 from the footprint [7, 10) by [7, 10)), the
    // assister contributes nothing: completion stays at tick 60.
    let (mut s, cmds) = start_plant((20.5, 8.5));
    run_plant(&mut s, &cmds, 60);
    assert!(!entity(&s, site).complete, "not complete after tick 59");
    step(&mut s, &[]);
    assert!(entity(&s, site).complete);
}

/// SIM-060: a factory builds its queue one item at a time, each spawning
/// at the factory's position, and drops an id outside its buildable set.
#[test]
fn sim_factory_queue_builds_in_order_at_its_position() {
    let mut factory = builder("factory", 20, &["tank"]);
    factory.categories = support::cats(&["STRUCTURE", "FACTORY"]);
    let mut tank = structure("tank", 100, 1000, 100);
    tank.categories = support::cats(&["MOBILE"]);
    let wld = world(vec![factory, tank], Vec::new(), flat_map(16));
    let mut s = rich(&wld);
    let f = s.spawn(Army(1), "factory", pos(6.5, 0.0, 6.5), q(100));
    let make = || Order::BuildFactory {
        blueprint: "tank".to_owned(),
    };
    let cmds = [
        order_cmd(0, 0, f, make(), QueueMode::Replace),
        order_cmd(0, 1, f, make(), QueueMode::Append),
    ];
    step(&mut s, &cmds); // tick 0
    let (first, second) = (EntityId(2), EntityId(3));
    let spawned = entity(&s, first);
    assert_eq!(spawned.pos, pos(6.5, 0.0, 6.5), "spawned at the factory");
    assert_eq!((spawned.built, spawned.complete), (Q::ZERO, false));
    assert!(!s.entities.contains_key(&second), "one item at a time");

    // per_tick(20) = 2 per tick: 100 / 2 = 50 work ticks.
    for _ in 1..50 {
        step(&mut s, &[]);
    }
    assert!(!entity(&s, first).complete, "not complete after tick 49");
    step(&mut s, &[]); // tick 50
    assert!(entity(&s, first).complete);
    let next = entity(&s, second);
    assert_eq!(
        (next.built, next.complete),
        (Q::ZERO, false),
        "started in the same phase"
    );
    assert_eq!(next.pos, pos(6.5, 0.0, 6.5));
    for _ in 51..100 {
        step(&mut s, &[]);
    }
    assert!(!entity(&s, second).complete, "not complete after tick 99");
    step(&mut s, &[]); // tick 100
    assert!(entity(&s, second).complete);
    assert_eq!(s.counters.builds_completed, 2);
    assert!(entity(&s, f).orders.is_empty());

    // An id outside the buildable set is dropped and counted.
    let before = s.counters.commands_ignored;
    let tick = s.tick.0;
    let bad = Order::BuildFactory {
        blueprint: "bomber".to_owned(),
    };
    step(&mut s, &[order_cmd(tick, 2, f, bad, QueueMode::Replace)]);
    assert_eq!(s.counters.commands_ignored, before + 1);
    assert!(entity(&s, f).orders.is_empty());
    assert_eq!(s.entities.len(), 3, "nothing spawned");
}

/// A world of `kinds` and `markers` on a flat 16 by 16 map.
fn site_world(kinds: Vec<UnitKind>, markers: Vec<Marker>) -> World {
    World::clone(&world(kinds, markers, flat_map(16)))
}

/// The rich state in `w` with a worker (id 1) at `at`.
fn site_in(w: World, at: (f64, f64)) -> State {
    let mut s = rich(&Arc::new(w));
    s.spawn(Army(1), "worker", pos(at.0, 0.0, at.1), q(100));
    s
}

fn site_state(kinds: Vec<UnitKind>, markers: Vec<Marker>, at: (f64, f64)) -> State {
    site_in(site_world(kinds, markers), at)
}

/// Command the worker to build `order` at the current tick and run one tick.
fn command(s: &mut State, order: Order) {
    let tick = s.tick.0;
    let seq = u32::try_from(tick).expect("a small tick");
    step(
        s,
        &[order_cmd(tick, seq, EntityId(1), order, QueueMode::Replace)],
    );
}

fn site_kinds() -> Vec<UnitKind> {
    let mut wall = structure("wall", 1, 1, 10);
    wall.footprint = Some((2, 2));
    let mut water = structure("pier", 1, 1, 10);
    water.build_on = supercom_sim::LayerSet::default();
    let mut tank = structure("tank", 1, 1, 10);
    tank.categories = support::cats(&["MOBILE"]);
    let mut slab = structure("slab", 1, 1, 10);
    slab.footprint = Some((0, 3));
    vec![
        builder("worker", 10, &["wall", "pier", "tank", "slab", "gen"]),
        wall,
        water,
        tank,
        slab,
        structure("gen", 1, 1, 10),
    ]
}

/// SIM-061: the site rules, one assertion each.
#[test]
fn sim_build_site_rules() {
    // The footprint default: a missing block or a zero dimension reads as 1.
    let kinds = site_kinds();
    let by_id = |id: &str| kinds.iter().find(|k| k.id == id).expect("a kind");
    assert_eq!(footprint_of(by_id("gen")), (1, 1));
    assert_eq!(footprint_of(by_id("slab")), (1, 3));
    assert_eq!(footprint_of(by_id("wall")), (2, 2));

    // In range: the worker 3 units from the site spawns it, with the
    // blueprint stored in lower case, y the height, and the order kept.
    let mut s = site_state(site_kinds(), Vec::new(), (4.5, 4.5));
    command(&mut s, build_at("GEN", 4.5, 8.5));
    let site = entity(&s, EntityId(2));
    assert_eq!(site.blueprint, "gen");
    assert_eq!(site.pos, pos(4.5, 0.0, 8.5));
    assert!(!site.complete);
    assert_eq!(entity(&s, EntityId(1)).work, Some(EntityId(2)));
    assert_eq!(s.counters.commands_ignored, 0);

    // Out of range (beyond 5 from the nearest footprint edge): the order
    // waits at the head, with no spawn and no work, and is not counted.
    let mut s = site_state(site_kinds(), Vec::new(), (4.5, 4.5));
    command(&mut s, build_at("gen", 4.5, 12.5));
    assert_eq!(s.entities.len(), 1);
    assert_eq!(entity(&s, EntityId(1)).orders.len(), 1);
    assert_eq!(entity(&s, EntityId(1)).work, None);
    assert_eq!(s.counters.commands_ignored, 0);
    // Moved into range, it starts on the next tick.
    s.entities.get_mut(&EntityId(1)).expect("worker").pos = pos(4.5, 0.0, 9.5);
    step(&mut s, &[]);
    assert_eq!(s.entities.len(), 2);

    // An impassable cell: dropped and counted.
    let mut w = site_world(site_kinds(), Vec::new());
    let width = usize::try_from(w.map.width).expect("a small map");
    w.map.passable.get_mut(&Layer::Land).expect("a land grid")[8 * width + 4] = false;
    let mut s = site_in(w, (4.5, 4.5));
    command(&mut s, build_at("gen", 4.5, 8.5));
    assert_eq!(s.entities.len(), 1);
    assert_eq!(s.counters.commands_ignored, 1);
    assert!(entity(&s, EntityId(1)).orders.is_empty());

    // An occupied cell (inside another entity's footprint): dropped.
    let mut s = site_state(site_kinds(), Vec::new(), (4.5, 4.5));
    s.spawn(Army(1), "gen", pos(4.5, 0.0, 8.5), q(100));
    command(&mut s, build_at("gen", 4.5, 8.5));
    assert_eq!(s.entities.len(), 2);
    assert_eq!(s.counters.commands_ignored, 1);

    // A footprint that overlaps one cell of another footprint: dropped; a
    // footprint of 2 by 2 centred at (4, 8) covers cells 3..5 by 7..9.
    let mut s = site_state(site_kinds(), Vec::new(), (4.5, 4.5));
    s.spawn(Army(1), "gen", pos(4.5, 0.0, 8.5), q(100)); // cell (4, 8)
    command(&mut s, build_at("wall", 4.0, 8.0));
    assert_eq!(s.entities.len(), 2);
    assert_eq!(s.counters.commands_ignored, 1);

    // A footprint partly outside the map: dropped.
    let mut s = site_state(site_kinds(), Vec::new(), (1.5, 4.5));
    command(&mut s, build_at("wall", 0.0, 4.0)); // cells -1..1
    assert_eq!(s.entities.len(), 1);
    assert_eq!(s.counters.commands_ignored, 1);

    // A MOBILE kind by BuildMobile: dropped.
    let mut s = site_state(site_kinds(), Vec::new(), (4.5, 4.5));
    command(&mut s, build_at("tank", 4.5, 8.5));
    assert_eq!(s.entities.len(), 1);
    assert_eq!(s.counters.commands_ignored, 1);

    // A kind without Land in build_on: dropped.
    let mut s = site_state(site_kinds(), Vec::new(), (4.5, 4.5));
    command(&mut s, build_at("pier", 4.5, 8.5));
    assert_eq!(s.entities.len(), 1);
    assert_eq!(s.counters.commands_ignored, 1);

    // A blueprint outside the builder's buildable set: dropped.
    let mut s = site_state(site_kinds(), Vec::new(), (4.5, 4.5));
    command(&mut s, build_at("nuke", 4.5, 8.5));
    assert_eq!(s.entities.len(), 1);
    assert_eq!(s.counters.commands_ignored, 1);

    // A Stop leaves the unfinished entity in place and clears the work.
    let mut s = site_state(site_kinds(), Vec::new(), (4.5, 4.5));
    command(&mut s, build_at("gen", 4.5, 8.5));
    step(&mut s, &[]);
    step(&mut s, &[]);
    let built = entity(&s, EntityId(2)).built;
    assert!(built > Q::ZERO);
    command(&mut s, Order::Stop);
    let site = entity(&s, EntityId(2));
    assert!(!site.complete);
    assert_eq!(entity(&s, EntityId(1)).work, None);
    assert!(entity(&s, EntityId(1)).orders.is_empty());
    step(&mut s, &[]);
    assert_eq!(entity(&s, EntityId(2)).built, built, "no resumption");
}

fn marker(name: &str, kind: MarkerKind, x: f64, z: f64) -> Marker {
    Marker {
        name: name.to_owned(),
        kind,
        pos: pos(x, 0.0, z),
    }
}

/// Shared shape of the extractor and plant tests: a deposit kind that
/// `restriction` restricts, a marker of `good` kind at (5.5, 5.5) and one of
/// the other kind at (12.5, 12.5).
fn deposit_state(restriction: BuildRestriction, good: MarkerKind, other: MarkerKind) -> State {
    let mut dep = restricted(structure("dep", 6, 60, 60), restriction);
    dep.maintenance_energy = Some(q(2));
    match restriction {
        BuildRestriction::OnHydrocarbonDeposit => dep.production_energy = Some(q(100)),
        _ => dep.production_mass = Some(q(2)),
    }
    site_state(
        vec![builder("worker", 10, &["dep"]), dep],
        vec![
            marker("good", good, 5.5, 5.5),
            marker("other", other, 12.5, 12.5),
        ],
        (8.5, 5.5),
    )
}

/// The deposit rules shared by SIM-041 and SIM-042.
fn deposit_rules(restriction: BuildRestriction, good: MarkerKind, other: MarkerKind) -> State {
    // Off every marker: dropped.
    let mut s = deposit_state(restriction, good, other);
    command(&mut s, build_at("dep", 9.5, 9.5));
    assert_eq!(s.entities.len(), 1, "off a marker");
    assert_eq!(s.counters.commands_ignored, 1);

    // On a marker of the other kind: dropped.
    let mut s = deposit_state(restriction, good, other);
    s.entities.get_mut(&EntityId(1)).expect("worker").pos = pos(12.5, 0.0, 9.5);
    command(&mut s, build_at("dep", 12.5, 12.5));
    assert_eq!(s.entities.len(), 1, "on the wrong kind of marker");
    assert_eq!(s.counters.commands_ignored, 1);

    // Exactly 1 away on x is not within 1: dropped; 0.5 away is.
    let mut s = deposit_state(restriction, good, other);
    command(&mut s, build_at("dep", 6.5, 5.5));
    assert_eq!(s.entities.len(), 1, "exactly one unit away");
    let mut s = deposit_state(restriction, good, other);
    command(&mut s, build_at("dep", 5.0, 5.5));
    assert_eq!(s.entities.len(), 2, "half a unit away");

    // On the marker: spawns; a second on the same marker is dropped.
    let mut s = deposit_state(restriction, good, other);
    command(&mut s, build_at("dep", 5.5, 5.5));
    assert_eq!(s.entities.len(), 2, "on the marker");
    let tick = s.tick.0;
    s.spawn(Army(1), "worker", pos(8.5, 0.0, 6.5), q(100));
    let second = order_cmd(
        tick,
        9,
        EntityId(3),
        build_at("dep", 4.9, 5.5),
        QueueMode::Replace,
    );
    step(&mut s, &[second]);
    assert_eq!(
        s.entities.len(),
        3,
        "no second deposit entity on the marker"
    );
    assert_eq!(s.counters.commands_ignored, 1);
    s
}

/// SIM-041: an extractor needs a Mass marker no other extractor holds, and
/// produces the kind's production: 2 per second is raw (2 << 32) / 10 =
/// 858993459 per tick, with the maintenance 2 requested as 858993459 energy.
#[test]
fn sim_extractor_requires_a_mass_marker() {
    let mut s = deposit_rules(
        BuildRestriction::OnMassDeposit,
        MarkerKind::Mass,
        MarkerKind::Hydrocarbon,
    );
    // The first deposit (id 2) completes sixty work ticks after tick 1's
    // start; run to completion and one more tick.
    for _ in 0..200 {
        if entity(&s, EntityId(2)).complete {
            break;
        }
        step(&mut s, &[]);
    }
    assert!(
        entity(&s, EntityId(2)).complete,
        "completes within 200 ticks"
    );
    step(&mut s, &[]);
    let l = army(&s).ledger;
    assert_eq!(l.income_mass.to_bits(), 858_993_459);
    assert_eq!(l.income_energy, Q::ZERO);
    assert_eq!(l.requested_energy.to_bits(), 858_993_459);
}

/// SIM-042: a hydrocarbon plant mirrors the extractor rule with Hydrocarbon
/// markers and produces the kind's energy: 100 per second is 10 per tick.
#[test]
fn sim_hydrocarbon_plant_on_marker() {
    let mut s = deposit_rules(
        BuildRestriction::OnHydrocarbonDeposit,
        MarkerKind::Hydrocarbon,
        MarkerKind::Mass,
    );
    for _ in 0..200 {
        if entity(&s, EntityId(2)).complete {
            break;
        }
        step(&mut s, &[]);
    }
    assert!(
        entity(&s, EntityId(2)).complete,
        "completes within 200 ticks"
    );
    step(&mut s, &[]);
    let l = army(&s).ledger;
    assert_eq!(l.income_energy, q(10));
    assert_eq!(l.requested_energy.to_bits(), 858_993_459);
}
