//! The minimum-radius back-off of an Attack (docs/specs/sim.md, SIM-086):
//! a mobile attacker inside its band floor steps back until its weapon can
//! fire. Synthetic; every distance is stated here, never read from the
//! helpers under test.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::collections::BTreeMap;

use supercom_sim::distance_xz;
use supercom_sim::{
    Army, EntityId, Layer, LayerSet, MapGrid, MotionType, Order, Q, QueueMode, State, UnitKind,
    WeaponKind,
};
use support::{flat_map, kind, order_cmd, pos, q, state, step, world};

const ROW: f64 = 20.5;
const ATTACKER_X: f64 = 20.5;

fn weapon(min_radius: Option<f64>, max_radius: f64) -> WeaponKind {
    WeaponKind {
        category: "Direct Fire".to_owned(),
        damage: q(1),
        damage_type: "Normal".to_owned(),
        rate_of_fire: Some(1.0),
        min_radius,
        max_radius: Some(max_radius),
        target_layers: LayerSet::from_caps("Land").expect("caps"),
        ..WeaponKind::default()
    }
}

/// A land unit with the given weapons; `mobile` false leaves it without a
/// motion type, so the movement phase never acts on it.
fn attacker_kind(weapons: Vec<WeaponKind>, mobile: bool) -> UnitKind {
    let mut k = kind("attacker", &["MOBILE"]);
    k.weapons = weapons;
    if mobile {
        k.motion = MotionType::Land;
        k.max_speed = Some(3.0);
    }
    k
}

fn band_weapon() -> WeaponKind {
    weapon(Some(5.0), 30.0)
}

struct Arena {
    s: State,
    attacker: EntityId,
    target: EntityId,
}

/// The attacker at `(x, ROW)` and an enemy structure `distance` due east of
/// it, with the attacker ordered to attack at tick 0 (the order is applied
/// by the first step).
fn arena_on(units: Vec<UnitKind>, map: MapGrid, x: f64, distance: f64) -> Arena {
    let w = world(units, Vec::new(), map);
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    s.add_army(Army(2), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
    let attacker = s.spawn(Army(1), "attacker", pos(x, 0.0, ROW), q(100));
    let target = s.spawn(Army(2), "dummy", pos(x + distance, 0.0, ROW), q(100));
    step(
        &mut s,
        &[order_cmd(
            0,
            0,
            attacker,
            Order::Attack { target },
            QueueMode::Replace,
        )],
    );
    Arena {
        s,
        attacker,
        target,
    }
}

fn arena(weapons: Vec<WeaponKind>, mobile: bool, distance: f64) -> Arena {
    arena_on(
        vec![
            attacker_kind(weapons, mobile),
            kind("dummy", &["STRUCTURE"]),
        ],
        flat_map(64),
        ATTACKER_X,
        distance,
    )
}

impl Arena {
    fn gap(&self) -> f64 {
        let a = self.s.entities.get(&self.attacker).expect("attacker");
        let t = self.s.entities.get(&self.target).expect("target");
        distance_xz(&a.pos, &t.pos)
    }

    fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            step(&mut self.s, &[]);
        }
    }

    fn shots(&self) -> u64 {
        self.s.counters.projectile_impacts
    }

    fn attacking(&self) -> bool {
        matches!(
            self.s
                .entities
                .get(&self.attacker)
                .and_then(|e| e.orders.front()),
            Some(Order::Attack { target }) if *target == self.target
        )
    }

    fn speed(&self) -> f64 {
        self.s.entities.get(&self.attacker).expect("attacker").speed
    }

    fn target_to(&mut self, x: f64) {
        self.s.entities.get_mut(&self.target).expect("target").pos = pos(x, 0.0, ROW);
    }
}

/// The first back-off scenario, run for 60 ticks and checked.
fn backed_off() -> Arena {
    let mut a = arena(vec![band_weapon()], true, 2.0);
    a.run(60);
    let d = a.gap();
    assert!(
        (5.0..=30.0).contains(&d),
        "backed off to a firing distance: {d}"
    );
    assert_eq!(a.speed().to_bits(), 0.0_f64.to_bits(), "at rest");
    assert!(a.attacking(), "the Attack is still at the head");
    assert!(a.shots() >= 1, "it fired from its new place");
    a
}

/// SIM-086: a mobile attacker inside its band floor backs off along the line
/// from the target until its weapon can fire, stops at the first tick its
/// distance reaches the floor, keeps the Attack, and fires. Red before the
/// change: the unchanged phase holds it at distance 2, where it cannot fire.
#[test]
fn sim_attack_backs_off_to_its_band_floor() {
    backed_off();
}

/// SIM-086: the decision is made afresh every tick, so a target that closes
/// again drives the attacker back out again. Red before the change for the
/// same reason as the first back-off test.
#[test]
fn sim_attacker_backs_off_again_when_the_target_closes() {
    let mut a = backed_off();
    let before = a.shots();
    let attacker_x = a.s.entities.get(&a.attacker).expect("attacker").pos.x;
    a.target_to(attacker_x + 1.0);
    assert!((a.gap() - 1.0).abs() < 1e-9);
    a.run(60);
    let d = a.gap();
    assert!((5.0..=30.0).contains(&d), "backed off again: {d}");
    assert!(a.shots() > before, "it fired again");
    assert!(a.attacking());
    assert_eq!(a.speed().to_bits(), 0.0_f64.to_bits());
}

/// SIM-081 unchanged: from outside the band the approach stops within the
/// range and never enters the band. Green before the change: the approach
/// is the existing rule, which the back-off must leave alone.
#[test]
fn sim_attack_approach_is_unchanged_outside_the_band() {
    let mut a = arena(vec![band_weapon()], true, 40.0);
    for _ in 0..120 {
        step(&mut a.s, &[]);
        assert!(a.gap() >= 5.0, "never inside the band: {}", a.gap());
    }
    assert!(a.gap() <= 30.0, "within range: {}", a.gap());
    assert!(a.shots() >= 1, "it fired");
    assert!(a.attacking());
}

/// SIM-086: an attacker the movement phase skips never moves and keeps the
/// Attack, firing once the target leaves its minimum radius. Green before
/// the change: the unchanged phase already skips it.
#[test]
fn sim_immobile_attacker_waits_inside_its_minimum() {
    let mut a = arena(vec![band_weapon()], false, 2.0);
    let start = a.s.entities.get(&a.attacker).expect("attacker").pos;
    a.run(30);
    let now = a.s.entities.get(&a.attacker).expect("attacker").pos;
    assert_eq!(now, start, "it never moves");
    assert!(a.attacking());
    assert_eq!(a.shots(), 0, "it cannot fire inside its minimum");
    a.target_to(ATTACKER_X + 10.0);
    a.run(20);
    assert!(
        a.shots() >= 1,
        "it fires once the target leaves the minimum"
    );
}

/// SIM-086: a kind with a minimum-free weapon has no band floor and never
/// backs off. Green before the change: the unchanged phase holds it, since
/// the longest range already covers distance 2.
#[test]
fn sim_mixed_kind_never_backs_off() {
    let mut a = arena(vec![band_weapon(), weapon(None, 12.0)], true, 2.0);
    let start = a.s.entities.get(&a.attacker).expect("attacker").pos;
    a.run(30);
    let now = a.s.entities.get(&a.attacker).expect("attacker").pos;
    assert_eq!(now, start, "it stays where it is");
    assert!(a.shots() >= 1, "the minimum-free weapon fires");
}

/// SIM-086: when the back-off goal lies off the map the attacker holds in
/// place with speed 0 and keeps the Attack, without counting an ignored
/// command. Green before the change, which holds it at distance 2; the
/// back-off must keep that hold.
#[test]
fn sim_back_off_without_a_route_holds_and_keeps_the_attack() {
    let mut a = arena_on(
        vec![
            attacker_kind(vec![band_weapon()], true),
            kind("dummy", &["STRUCTURE"]),
        ],
        flat_map(64),
        1.5,
        2.0,
    );
    let start = a.s.entities.get(&a.attacker).expect("attacker").pos;
    let ignored = a.s.counters.commands_ignored;
    a.run(30);
    let now = a.s.entities.get(&a.attacker).expect("attacker").pos;
    assert_eq!(now, start, "it holds in place");
    assert_eq!(a.speed().to_bits(), 0.0_f64.to_bits());
    assert!(a.attacking());
    assert_eq!(a.s.counters.commands_ignored, ignored);
}

/// SIM-086: when the goal's cell is impassable the attacker holds, keeps the
/// Attack, and fires once the target leaves the floor. Green before the
/// change, which holds it at distance 2; the back-off must neither move it
/// to a neighbouring cell nor drop the Attack.
#[test]
fn sim_back_off_into_an_impassable_cell_holds() {
    let (width, height) = (64_u32, 64_u32);
    let cells = (width * height) as usize;
    // The goal point is 22.5 - (5 + 1) = 16.5 on row 20: cell (16, 20).
    let mut land = vec![true; cells];
    land[(20 * width + 16) as usize] = false;
    let mut passable = BTreeMap::new();
    passable.insert(Layer::Land, land);
    passable.insert(Layer::Air, vec![true; cells]);
    let map = MapGrid {
        width,
        height,
        heights: vec![Q::ZERO; ((width + 1) * (height + 1)) as usize],
        passable,
        water: None,
    };
    let mut a = arena_on(
        vec![
            attacker_kind(vec![band_weapon()], true),
            kind("dummy", &["STRUCTURE"]),
        ],
        map,
        ATTACKER_X,
        2.0,
    );
    let start = a.s.entities.get(&a.attacker).expect("attacker").pos;
    a.run(30);
    let now = a.s.entities.get(&a.attacker).expect("attacker").pos;
    assert_eq!(now, start, "it holds in place");
    assert_eq!(a.speed().to_bits(), 0.0_f64.to_bits());
    assert!(a.attacking());
    a.target_to(ATTACKER_X + 10.0);
    a.run(20);
    assert!(a.shots() >= 1, "it fires once the target leaves the floor");
}

/// SIM-086: a diagonal back-off, from planar distance 1.9 along x = z
/// toward the origin, whose goal cell's centre lies close enough to the
/// floor that the mover reaches the arrival radius of its goal while its
/// distance is still below the floor. Green at the tip. It pins that only
/// arrival clears the goal and route and zeroes the speed (so the arrival
/// branch is reached below the floor, which keeps the test from passing
/// vacuously), that the Attack survives that arrival, that an entity
/// standing in its enterable goal cell walks to the cell's centre rather
/// than holding, and that it ends 5 to 30 away, at rest, having fired.
/// Red against a mutant that pops the Attack at arrival and against one
/// that holds inside the goal cell.
#[test]
fn sim_diagonal_back_off_arrives_below_the_floor_and_keeps_the_attack() {
    let arena_world = world(
        vec![
            attacker_kind(vec![band_weapon()], true),
            kind("dummy", &["STRUCTURE"]),
        ],
        Vec::new(),
        flat_map(64),
    );
    let mut sim = state(&arena_world, Q::ZERO, Q::ZERO);
    sim.add_army(Army(2), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
    let corner = 24.2427_f64;
    let off = 1.9 / core::f64::consts::SQRT_2;
    let attacker = sim.spawn(
        Army(1),
        "attacker",
        pos(corner - off, 0.0, corner - off),
        q(100),
    );
    let target = sim.spawn(Army(2), "dummy", pos(corner, 0.0, corner), q(100));
    step(
        &mut sim,
        &[order_cmd(
            0,
            0,
            attacker,
            Order::Attack { target },
            QueueMode::Replace,
        )],
    );
    let gap = |sim: &State| {
        let att = sim.entities.get(&attacker).expect("attacker");
        let goal_mark = sim.entities.get(&target).expect("target");
        distance_xz(&att.pos, &goal_mark.pos)
    };
    let mut arrived_below_floor = false;
    for _ in 0..60 {
        let before = gap(&sim);
        step(&mut sim, &[]);
        let att = sim.entities.get(&attacker).expect("attacker");
        if before < 4.95 && att.goal.is_none() && att.route.is_empty() && att.speed == 0.0 {
            arrived_below_floor = true;
        }
    }
    assert!(
        arrived_below_floor,
        "the arrival branch was reached below the floor"
    );
    let att = sim.entities.get(&attacker).expect("attacker");
    assert!(
        matches!(att.orders.front(), Some(Order::Attack { target: head }) if *head == target),
        "the Attack is still at the head"
    );
    let dist = gap(&sim);
    assert!(
        (5.0..=30.0).contains(&dist),
        "backed off to att firing distance: {dist}"
    );
    assert_eq!(att.speed.to_bits(), 0.0_f64.to_bits(), "at rest");
    assert!(sim.counters.projectile_impacts >= 1, "it fired");
}
