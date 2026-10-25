//! The defeat kill rule and the civilian exemption on synthetic worlds
//! (docs/specs/sim.md, SIM-084, SIM-085, SIM-087;
//! docs/adr/0014-vision-and-victory.md). No install, no file. Every expected
//! id, count, and tick is stated beside its assertion from the scenario's
//! construction, never from the helpers under test.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::collections::BTreeSet;
use std::sync::Arc;

use supercom_sim::{
    Army, Command, EntityId, MotionType, Order, Outcome, Q, QueueMode, State, Tick, UnitKind,
    WeaponKind,
};
use supercom_sim::{LayerSet, ProjectileKind, World};
use support::{flat_map, kind, pos, q, state, step, structure, world};

/// The ticks between a defeat and the kill (SIM-087).
const DELAY: u64 = 200;

/// A kind that sees far enough for every unit of a scenario to see the rest.
fn seer(id: &str, categories: &[&str]) -> UnitKind {
    let mut k = kind(id, categories);
    k.vision_radius = Some(500.0);
    k
}

fn mover() -> UnitKind {
    let mut k = seer("grunt", &["MOBILE"]);
    k.motion = MotionType::Land;
    k.max_speed = Some(1.0);
    k
}

/// An immobile kind with a short weapon: it can hold an Attack order on a
/// target it can never reach, so the order stays at the head of its queue.
fn turret() -> UnitKind {
    let mut k = kind("turret", &["STRUCTURE"]);
    k.weapons = vec![WeaponKind {
        category: "Direct Fire".to_owned(),
        damage: q(1),
        damage_type: "Normal".to_owned(),
        rate_of_fire: Some(1.0),
        max_radius: Some(5.0),
        muzzle_velocity: Some(10.0),
        ..WeaponKind::default()
    }];
    k
}

/// A state with armies 1 to `armies`, each owning a commander, over kinds
/// for a commander, a plain mobile unit, a wall, and a buildable site.
fn scenario(armies: u8) -> (State, Vec<EntityId>) {
    let w = world(
        vec![
            seer("cmd", &["MOBILE", "COMMAND"]),
            mover(),
            turret(),
            seer("wall", &["STRUCTURE", "WALL"]),
            structure("site", 1, 1, 10),
        ],
        Vec::new(),
        flat_map(128),
    );
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let mut ids = Vec::new();
    for n in 1..=armies {
        if n > 1 {
            s.add_army(Army(n), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
        }
        ids.push(s.spawn(Army(n), "cmd", pos(f64::from(n) * 10.0, 0.0, 5.0), q(100)));
    }
    (s, ids)
}

fn kill(s: &mut State, id: EntityId) {
    s.entities.get_mut(&id).expect("the entity exists").health = Q::ZERO;
}

fn order(army: u8, unit: EntityId, order: Order) -> Command {
    Command {
        tick: Tick(0),
        source: 0,
        seq: u32::from(army),
        army: Army(army),
        units: vec![unit],
        order,
        mode: QueueMode::Replace,
    }
}

fn ids(s: &State) -> BTreeSet<EntityId> {
    s.entities.keys().copied().collect()
}

/// Step until the tick counter reads `tick`, with no commands.
fn run_to(s: &mut State, tick: u64) {
    while s.tick.0 < tick {
        step(s, &[]);
    }
}

/// The values the kill must leave alone.
fn standing(s: &State) -> (Option<Outcome>, u64, u64) {
    (s.outcome, s.counters.defeats, s.counters.victories)
}

/// SIM-087: 200 ticks after the victory phase defeats an army, the death
/// phase removes every entity of the army outside the WALL category,
/// finished or not, once, counted apart from deaths.
#[test]
fn sim_defeated_army_is_killed_after_200_ticks() {
    let (mut s, cmds) = scenario(3);
    let grunt3 = s.spawn(Army(3), "grunt", pos(60.0, 0.0, 60.0), q(100));
    let site3 = s
        .spawn_under_construction(Army(3), "site", pos(70.0, 0.0, 70.0))
        .expect("a buildable kind");
    let wall3 = s.spawn(Army(3), "wall", pos(80.0, 0.0, 80.0), q(100));
    let grunt1 = s.spawn(Army(1), "turret", pos(20.0, 0.0, 20.0), q(100));
    let before = ids(&s);

    // Tick 0: army 3 queues a move, army 1's unit queues an attack on army
    // 3's, and army 3's commander dies, which defeats the army in that tick.
    kill(&mut s, cmds[2]);
    let t = s.tick.0;
    assert_eq!(t, 0);
    step(
        &mut s,
        &[
            order(
                3,
                grunt3,
                Order::Move {
                    to: pos(120.0, 0.0, 120.0),
                },
            ),
            order(1, grunt1, Order::Attack { target: grunt3 }),
        ],
    );
    assert_eq!(s.armies[&Army(3)].defeated, Some(Tick(t)));
    assert_eq!((s.counters.deaths, s.counters.kills), (1, 0));
    let held = standing(&s);
    assert_eq!(held, (None, 1, 0), "two armies still stand");

    // At T + 199 the three remaining entities of army 3 exist.
    run_to(&mut s, t + DELAY);
    for id in [grunt3, site3, wall3] {
        assert!(s.entities.contains_key(&id), "{id:?} exists at T + 199");
    }
    assert_eq!(s.counters.kills, 0);
    assert!(!s.entities[&grunt3].orders.is_empty(), "a queued order");
    assert!(
        matches!(s.entities[&grunt1].orders.front(), Some(Order::Attack { target }) if *target == grunt3),
        "the enemy still holds its attack"
    );
    assert!(s.is_visible(Army(1), grunt3));
    assert!(s.is_visible(Army(3), site3));
    let deaths = s.counters.deaths;

    // The step of T + 200 removes the plain unit and the site, not the wall.
    assert_eq!(s.tick.0, t + DELAY);
    step(&mut s, &[]);
    let mut after = before.clone();
    after.remove(&cmds[2]);
    after.remove(&grunt3);
    after.remove(&site3);
    assert_eq!(ids(&s), after, "the wall alone of army 3 remains");
    assert!(s.entities.contains_key(&wall3));
    assert_eq!(s.counters.kills, 2);
    assert_eq!(s.counters.deaths, deaths, "a kill is not a death");
    assert!(
        s.entities[&grunt1].orders.is_empty(),
        "the attack naming a killed entity is popped"
    );
    for army in 1..=3 {
        for id in [grunt3, site3] {
            assert!(
                !s.is_visible(Army(army), id) && !s.visible(Army(army)).contains(&id),
                "army {army} still sees {id:?}"
            );
        }
    }
    assert_eq!(standing(&s), held, "the kill changes no outcome or count");

    // The kill fires once: nothing more is removed, and a unit spawned after
    // it is never hunted.
    step(&mut s, &[]);
    assert_eq!((ids(&s), s.counters.kills), (after.clone(), 2));
    let late = s.spawn(Army(3), "grunt", pos(90.0, 0.0, 90.0), q(100));
    run_to(&mut s, t + 2 * DELAY);
    assert!(s.entities.contains_key(&late), "never hunted");
    assert_eq!(s.counters.kills, 2);
    assert_eq!(standing(&s), held);
}

/// SIM-087 with SIM-085: in a two-army match the outcome is decided at the
/// defeat, and the kill follows it.
#[test]
fn sim_kill_follows_the_outcome_in_a_two_army_match() {
    let (mut s, cmds) = scenario(2);
    let grunt2 = s.spawn(Army(2), "grunt", pos(40.0, 0.0, 40.0), q(100));
    kill(&mut s, cmds[1]);
    let t = s.tick.0;
    step(&mut s, &[]);
    let held = (Some(Outcome::Winner(Army(1))), 1, 1);
    assert_eq!(standing(&s), held, "decided at the defeat tick");
    assert_eq!(s.armies[&Army(2)].defeated, Some(Tick(t)));

    run_to(&mut s, t + DELAY);
    assert!(s.entities.contains_key(&grunt2), "present at T + 199");
    assert_eq!(s.counters.kills, 0);
    step(&mut s, &[]);
    assert!(!s.entities.contains_key(&grunt2), "gone after T + 200");
    assert_eq!(s.counters.kills, 1);
    assert_eq!(standing(&s), held);
    let tick = s.tick.0;
    for _ in 0..5 {
        step(&mut s, &[]);
    }
    assert_eq!(s.tick.0, tick + 5, "the tick runs on");
    assert_eq!((standing(&s), s.counters.kills), (held, 1));
}

/// SIM-084 and SIM-085: a civilian army is never defeated, never counted,
/// and never killed.
#[test]
fn sim_civilian_army_is_never_defeated_or_counted() {
    let (mut s, cmds) = scenario(2);
    s.add_civilian_army(Army(3), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
    let grunt3 = s.spawn(Army(3), "grunt", pos(60.0, 0.0, 60.0), q(100));
    let grunt1 = s.spawn(Army(1), "grunt", pos(20.0, 0.0, 20.0), q(100));
    assert!(s.armies[&Army(3)].civilian);
    assert!(!s.armies[&Army(1)].civilian);
    let moved = order(
        3,
        grunt3,
        Order::Move {
            to: pos(100.0, 0.0, 100.0),
        },
    );
    let mut ignored = None;
    for tick in 0..50 {
        let commands = if tick == 0 {
            vec![moved.clone()]
        } else {
            Vec::new()
        };
        step(&mut s, &commands);
        assert!(
            s.armies[&Army(3)].defeated.is_none() && !s.snapshot().armies[2].defeated,
            "the civilian army is defeated at tick {tick}"
        );
        ignored.get_or_insert(s.counters.commands_ignored);
        assert_eq!(Some(s.counters.commands_ignored), ignored);
    }
    assert_eq!(ignored, Some(0));
    let civilian = &s.snapshot().armies[2];
    assert!(civilian.civilian && civilian.defeated_at.is_none());
    assert_eq!(s.counters.commands_applied, 1, "its Move applies");
    assert!(!s.entities[&grunt3].orders.is_empty());
    assert_eq!(standing(&s), (None, 0, 0));

    // Army 1 loses its commander: army 2 is the last standing, the civilian
    // army still exists, undefeated and uncounted.
    kill(&mut s, cmds[0]);
    let t = s.tick.0;
    step(&mut s, &[]);
    assert_eq!(s.outcome, Some(Outcome::Winner(Army(2))));
    assert_eq!((s.counters.defeats, s.counters.victories), (1, 1));
    assert!(s.armies[&Army(3)].defeated.is_none());

    // The kill takes army 1's unit and never the civilian army's.
    run_to(&mut s, t + DELAY + 1);
    assert!(!s.entities.contains_key(&grunt1));
    assert!(s.entities.contains_key(&grunt3));
    assert_eq!(s.counters.kills, 1);
    assert!(s.armies[&Army(3)].defeated.is_none());

    // One commanded and one civilian army decide nothing.
    let (mut s, _) = scenario(1);
    s.add_civilian_army(Army(2), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
    let grunt2 = s.spawn(Army(2), "grunt", pos(60.0, 0.0, 60.0), q(100));
    for _ in 0..50 {
        step(&mut s, &[]);
    }
    assert_eq!(standing(&s), (None, 0, 0));
    assert!(s.armies.values().all(|a| a.defeated.is_none()));
    assert!(s.entities.contains_key(&grunt2));
}

/// SIM-087: the kill counts from the tick that defeated the army, not from
/// tick 0, and the snapshot reports that tick and the army's civilian mark.
#[test]
fn sim_kill_counts_from_the_stored_defeat_tick() {
    let (mut s, cmds) = scenario(2);
    let grunt = s.spawn(Army(2), "grunt", pos(40.0, 0.0, 40.0), q(100));
    let turret = s.spawn(Army(2), "turret", pos(45.0, 0.0, 45.0), q(100));
    let wall = s.spawn(Army(2), "wall", pos(50.0, 0.0, 50.0), q(100));
    run_to(&mut s, 37);
    kill(&mut s, cmds[1]);
    step(&mut s, &[]);
    let t = 37;
    assert_eq!(s.armies[&Army(2)].defeated, Some(Tick(t)));
    let entry = s
        .snapshot()
        .armies
        .into_iter()
        .find(|a| a.id == Army(2))
        .expect("army 2 in the snapshot");
    assert!(entry.defeated && !entry.civilian);
    assert_eq!(entry.defeated_at, Some(Tick(t)));

    run_to(&mut s, t + DELAY);
    for id in [grunt, turret, wall] {
        assert!(s.entities.contains_key(&id), "{id:?} exists at T + 199");
    }
    let kills = s.counters.kills;
    step(&mut s, &[]);
    assert!(!s.entities.contains_key(&grunt) && !s.entities.contains_key(&turret));
    assert!(s.entities.contains_key(&wall), "the wall is spared");
    assert_eq!(s.counters.kills, kills + 2);
}

/// SIM-087 with SIM-080: the kill runs after the death phase's own removals,
/// so an entity of the defeated army that dies by damage in the kill's tick
/// is a death, not a kill, and is counted once.
#[test]
fn sim_kill_follows_the_death_phase_removals() {
    let (mut s, cmds) = scenario(2);
    let dying = s.spawn(Army(2), "grunt", pos(40.0, 0.0, 40.0), q(100));
    let doomed = s.spawn(Army(2), "grunt", pos(50.0, 0.0, 50.0), q(100));
    kill(&mut s, cmds[1]);
    let t = s.tick.0;
    step(&mut s, &[]);
    assert_eq!(s.armies[&Army(2)].defeated, Some(Tick(t)));
    run_to(&mut s, t + DELAY);
    let (deaths, kills) = (s.counters.deaths, s.counters.kills);
    assert_eq!(kills, 0);
    kill(&mut s, dying);
    step(&mut s, &[]);
    assert!(!s.entities.contains_key(&dying) && !s.entities.contains_key(&doomed));
    assert_eq!(s.counters.deaths, deaths + 1, "the dying unit is a death");
    assert_eq!(s.counters.kills, 1, "the kill takes only the other unit");
}

/// SIM-087: the kill removes every entity of the defeated army outside the
/// WALL category, and the exemption is read from the entity's own kind: a
/// kind the world does not hold has no WALL mark, so the entity is present
/// after the step of tick T + 199, removed after the step of T + 200, and
/// counted as one kill (the defeated commander is a death, not a kill).
#[test]
fn sim_kill_takes_an_entity_whose_kind_is_absent() {
    let (mut s, cmds) = scenario(2);
    let ghost = s.spawn(Army(2), "absent-kind", pos(40.0, 0.0, 40.0), q(100));
    kill(&mut s, cmds[1]);
    let t = s.tick.0;
    step(&mut s, &[]);
    assert_eq!(s.armies[&Army(2)].defeated, Some(Tick(t)));
    run_to(&mut s, t + DELAY);
    assert!(s.entities.contains_key(&ghost), "present at T + 199");
    assert_eq!(s.counters.kills, 0);
    step(&mut s, &[]);
    assert!(!s.entities.contains_key(&ghost), "gone after T + 200");
    assert_eq!(s.counters.kills, 1, "the one entity is the one kill");
}

/// SIM-087: the kill removes entities only; a projectile aimed at one is
/// left in flight. A gun of the surviving army 1 holds the nearest enemy in
/// range, a unit of army 3 (defeated at tick T = 0) 30 units away, and fires
/// at ticks 0 and 195 (a cooldown of ceil(10 / 0.0514) = 195 ticks) with a
/// shot of speed 10, one unit a tick. The first shot lands near tick 30. The
/// second, launched at 195, is in the air across the step of T + 200, which
/// removes its target; it stays under the same id with its aim unchanged,
/// lands at that aim near tick 225 counting a second impact, and is gone.
#[test]
fn sim_kill_leaves_a_projectile_in_flight() {
    let mut w = World::empty();
    let mut gun = kind("gun", &["STRUCTURE"]);
    gun.vision_radius = Some(500.0);
    gun.weapons = vec![WeaponKind {
        category: "Direct Fire".to_owned(),
        damage: q(1),
        damage_type: "Normal".to_owned(),
        rate_of_fire: Some(0.0514),
        max_radius: Some(100.0),
        muzzle_velocity: Some(10.0),
        projectile_id: Some("slug".to_owned()),
        target_layers: LayerSet::from_caps("Land").expect("caps"),
        ..WeaponKind::default()
    }];
    w.units = [seer("cmd", &["MOBILE", "COMMAND"]), mover(), gun]
        .into_iter()
        .map(|k| (k.id.clone(), k))
        .collect();
    w.projectiles.insert(
        "slug".to_owned(),
        ProjectileKind {
            id: "slug".to_owned(),
            ..ProjectileKind::default()
        },
    );
    w.map = flat_map(128);
    let w = Arc::new(w);
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    for n in 2..=3 {
        s.add_army(Army(n), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
    }
    let cmds: Vec<EntityId> = (1..=3_u8)
        .map(|n| s.spawn(Army(n), "cmd", pos(f64::from(n) * 10.0, 0.0, 5.0), q(100)))
        .collect();
    s.spawn(Army(1), "gun", pos(20.0, 0.0, 50.0), q(100));
    let victim = s.spawn(Army(3), "grunt", pos(50.0, 0.0, 50.0), q(100));
    kill(&mut s, cmds[2]);
    let t = s.tick.0;
    step(&mut s, &[]);
    assert_eq!(s.armies[&Army(3)].defeated, Some(Tick(t)));

    run_to(&mut s, t + DELAY);
    assert!(s.entities.contains_key(&victim), "present at T + 199");
    let (id, before) = {
        let mut in_air = s.projectiles.iter();
        let first = in_air.next().expect("a shot in the air at T + 199");
        assert!(in_air.next().is_none(), "exactly one shot");
        assert_eq!(first.1.target, Some(victim));
        (*first.0, first.1.clone())
    };
    let landed = s.counters.projectile_impacts;
    assert_eq!(landed, 1, "the first shot has landed");

    step(&mut s, &[]);
    assert!(
        !s.entities.contains_key(&victim),
        "the kill took the target"
    );
    let after = s
        .projectiles
        .get(&id)
        .expect("the shot survives its target's removal");
    assert_eq!(
        [after.aim.x, after.aim.y, after.aim.z].map(f64::to_bits),
        [before.aim.x, before.aim.y, before.aim.z].map(f64::to_bits),
        "the aim is untouched"
    );
    assert_eq!(s.counters.projectile_impacts, landed, "still in the air");

    run_to(&mut s, t + DELAY + 40);
    assert!(s.projectiles.is_empty(), "the shot is gone");
    assert_eq!(s.counters.projectile_impacts, landed + 1, "it landed once");
}
