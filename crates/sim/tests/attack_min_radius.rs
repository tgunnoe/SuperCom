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

/// The attacker at `(attacker_x, ROW)` and an enemy structure at
/// `(target_x, ROW)`, on the 64-cell flat map, ordered to attack as in
/// [`arena_on`]. `arena_on` puts the target due east and its distance is
/// not negative, so a target to the west of the attacker needs this one.
fn arena_between(weapons: Vec<WeaponKind>, attacker_x: f64, target_x: f64) -> Arena {
    let w = world(
        vec![attacker_kind(weapons, true), kind("dummy", &["STRUCTURE"])],
        Vec::new(),
        flat_map(64),
    );
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    s.add_army(Army(2), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
    let attacker = s.spawn(Army(1), "attacker", pos(attacker_x, 0.0, ROW), q(100));
    let target = s.spawn(Army(2), "dummy", pos(target_x, 0.0, ROW), q(100));
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

impl Arena {
    fn at(&self) -> (u64, u64) {
        let p = self.s.entities.get(&self.attacker).expect("attacker").pos;
        (p.x.to_bits(), p.z.to_bits())
    }

    fn at_rest_in_band(&self, what: &str) {
        let d = self.gap();
        assert!((5.0..=30.0).contains(&d), "{what}: a firing distance: {d}");
        assert_eq!(self.speed().to_bits(), 0.0_f64.to_bits(), "{what}: at rest");
        assert!(self.attacking(), "{what}: the Attack is still at the head");
        assert!(self.shots() >= 1, "{what}: it fired");
    }
}

/// SIM-086: the floor is the smallest minimum among the active weapons. With
/// minimums 5 and 8 (both maximum 30) the attacker, from distance 2, stops
/// at the first tick its distance reaches 5, so it rests in [5, 8); the
/// largest minimum, 8, would carry it past 8. Red against a mutant that keeps
/// the largest minimum.
#[test]
fn sim_band_floor_is_the_smallest_minimum() {
    let mut a = arena(
        vec![weapon(Some(5.0), 30.0), weapon(Some(8.0), 30.0)],
        true,
        2.0,
    );
    a.run(60);
    a.at_rest_in_band("two minimums");
    let d = a.gap();
    assert!(
        (5.0..8.0).contains(&d),
        "rests below the larger minimum: {d}"
    );
}

/// SIM-086: a floor not below the attack range is not honoured. One weapon
/// with minimum 40 and maximum 30 can never fire, so the attacker, from
/// distance 2, neither backs off nor re-plans. The attacker stands 2 east
/// of the target, so a floor of 40 would put the back-off goal at x 51.5,
/// on the map; west of the target the goal would lie off the map and the
/// attacker would hold anyway. Red against a mutant that drops the
/// floor-below-range filter.
#[test]
fn sim_floor_at_or_above_the_range_is_not_honoured() {
    let mut a = arena_between(vec![weapon(Some(40.0), 30.0)], 12.5, 10.5);
    let start = a.at();
    let plans = a.s.plans;
    a.run(30);
    assert_eq!(a.at(), start, "it keeps its position bit for bit");
    assert_eq!(a.s.plans, plans, "no plan is made");
    assert_eq!(a.speed().to_bits(), 0.0_f64.to_bits());
    assert!(a.attacking());
}

/// SIM-086 with SIM-075: a manual-fire weapon and a Death-category weapon,
/// each with maximum 12, no minimum, and a rate of fire, are not active, so
/// they do not void the banded weapon's floor: the attacker, from distance
/// 2, backs off and fires. Red against a mutant that counts inactive weapons.
#[test]
fn sim_inactive_weapons_do_not_void_the_floor() {
    let manual = WeaponKind {
        manual_fire: true,
        ..weapon(None, 12.0)
    };
    let death = WeaponKind {
        category: "Death".to_owned(),
        ..weapon(None, 12.0)
    };
    let mut a = arena(vec![band_weapon(), manual, death], true, 2.0);
    a.run(60);
    a.at_rest_in_band("inactive weapons");
}

/// SIM-086: an active weapon with no maximum radius cannot take part in the
/// floor either way: it is skipped, so the banded weapon's floor stands and
/// the attacker, from distance 2, backs off and fires. Red against a mutant
/// that lets a weapon without a maximum void the floor.
#[test]
fn sim_weapon_without_a_maximum_is_skipped_by_the_floor() {
    let unbounded = WeaponKind {
        max_radius: None,
        ..weapon(None, 30.0)
    };
    let mut a = arena(vec![band_weapon(), unbounded], true, 2.0);
    a.run(60);
    a.at_rest_in_band("a weapon without a maximum");
}

/// SIM-086: the floor is exclusive. Placed at planar distance exactly 5.0
/// (dx 5.0, dz 0.0 in binary64: 20.5 and 25.5 are exact) the attacker has
/// reached its floor, so it holds still, at rest, and fires. Red against a
/// mutant that makes the comparison inclusive.
#[test]
fn sim_attacker_at_exactly_the_floor_holds_and_fires() {
    let mut a = arena_between(vec![band_weapon()], ATTACKER_X, ATTACKER_X + 5.0);
    assert_eq!(a.gap().to_bits(), 5.0_f64.to_bits(), "exactly the floor");
    let start = a.at();
    assert_eq!(start, (ATTACKER_X.to_bits(), ROW.to_bits()));
    a.run(30);
    assert_eq!(a.at(), start, "it holds bit for bit");
    assert_eq!(a.speed().to_bits(), 0.0_f64.to_bits());
    assert!(a.attacking());
    assert!(a.shots() >= 1, "it fires");
}

/// SIM-086: an attacker standing on its target's position has no direction
/// away from it, and takes +x. Spawned at the target's position (32.5, ROW),
/// 20.5 or more cells from every edge of the 64-cell map, it rests with x at
/// least 5 above the target's. Red against a mutant that flips the direction
/// to -x.
#[test]
fn sim_coincident_attacker_backs_off_along_plus_x() {
    let mut a = arena_between(vec![band_weapon()], 32.5, 32.5);
    a.run(60);
    a.at_rest_in_band("coincident");
    let attacker_x = a.s.entities.get(&a.attacker).expect("attacker").pos.x;
    assert!(
        attacker_x - 32.5 >= 5.0,
        "east of the target by the floor or more: {attacker_x}"
    );
}

/// SIM-086: the plan is remade only when the goal's cell changes. The
/// attacker 2 east of a target on the row's centre line has the same goal
/// cell (26) on every tick of its way east, so exactly one plan is made from
/// the Attack's application to rest (the first step applies it and plans in
/// the same tick, so the count is 1 when the arena returns), and none in
/// the 30 ticks after. Red against a mutant that plans every tick.
#[test]
fn sim_back_off_plans_once_per_goal_cell() {
    let mut a = arena_between(vec![band_weapon()], 22.5, 20.5);
    assert_eq!(a.s.plans, 1, "one plan from the first back-off tick");
    a.run(60);
    a.at_rest_in_band("plan count");
    assert_eq!(a.s.plans, 1, "still one plan at rest");
    a.run(30);
    assert_eq!(a.s.plans, 1, "no plan after rest");
}

/// SIM-086: a back-off goal beyond the map's far edge is a hold. At x 62.5
/// with the target 2 to the west the goal point 62.5 - 2 + 6 = 66.5 lies in
/// column 66 of 64: the attacker keeps its position bit for bit, speed 0,
/// no route, no goal, keeps the Attack, and ignores no command. Red against a mutant
/// that drops the map's upper bound.
#[test]
fn sim_back_off_beyond_the_far_edge_holds() {
    let mut a = arena_between(vec![band_weapon()], 62.5, 60.5);
    let start = a.at();
    let ignored = a.s.counters.commands_ignored;
    a.run(30);
    let e = a.s.entities.get(&a.attacker).expect("attacker");
    assert_eq!(a.at(), start, "it holds in place");
    assert_eq!(e.speed.to_bits(), 0.0_f64.to_bits());
    assert!(e.route.is_empty(), "no route");
    assert!(e.goal.is_none(), "no goal");
    assert!(a.attacking());
    assert_eq!(a.s.counters.commands_ignored, ignored);
}

/// SIM-086: the goal lies one unit beyond the floor. With the target at x
/// 32.9 and the attacker 2 east of it on the row's centre, the goal point
/// 32.9 + 5 + 1 = 38.9 lies in cell 38, whose centre 38.5 is 5.6 from the
/// target; a margin of 0 gives point 37.9, cell 37, centre 37.5, 4.6 from the
/// target, inside the floor. The stored goal after the first back-off tick
/// is at least 5 from the target, and the attacker rests 5 to 30 away.
#[test]
fn sim_back_off_goal_keeps_the_one_unit_margin() {
    let mut a = arena_between(vec![band_weapon()], 34.9, 32.9);
    let goal =
        a.s.entities
            .get(&a.attacker)
            .and_then(|e| e.goal)
            .expect("a goal after the first back-off tick");
    let from_target = distance_xz(&goal, &pos(32.9, 0.0, ROW));
    assert!(
        from_target >= 5.0,
        "the goal cell's centre is 5.6 from the target: {from_target}"
    );
    a.run(60);
    a.at_rest_in_band("margin");
}
