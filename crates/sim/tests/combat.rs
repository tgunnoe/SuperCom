//! The combat rules (docs/specs/sim.md, SIM-075 to SIM-081): acquisition,
//! the firing clock, projectile flight, impact damage with armour,
//! regeneration, death, and the Attack order. All synthetic; every expected
//! value is stated or computed here, never by the helpers under test.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::manual_midpoint,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use supercom_sim::distance_xz;
use supercom_sim::{
    Army, EntityId, Layer, LayerSet, MapGrid, MotionType, Order, Pos, Projectile, ProjectileId,
    ProjectileKind, Q, QueueMode, State, UnitKind, WeaponKind, World,
};
use support::{flat_map, kind, order_cmd, pos, q, step};

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} is not {b}");
}

fn gun(category: &str, max_radius: f64, rate: f64, damage: i64) -> WeaponKind {
    WeaponKind {
        category: category.to_owned(),
        damage: q(damage),
        damage_type: "Normal".to_owned(),
        rate_of_fire: Some(rate),
        max_radius: Some(max_radius),
        target_layers: LayerSet::from_caps("Land").expect("caps"),
        ..WeaponKind::default()
    }
}

fn armed(id: &str, weapons: Vec<WeaponKind>) -> UnitKind {
    let mut k = kind(id, &["STRUCTURE"]);
    k.weapons = weapons;
    k
}

fn shot_kind(id: &str, speed: f64) -> ProjectileKind {
    ProjectileKind {
        id: id.to_owned(),
        initial_speed: Some(speed),
        ..ProjectileKind::default()
    }
}

fn arena(
    units: Vec<UnitKind>,
    projectiles: Vec<ProjectileKind>,
    map: MapGrid,
    armour: BTreeMap<String, BTreeMap<String, Q>>,
) -> State {
    let mut w = World::empty();
    w.units = units.into_iter().map(|k| (k.id.clone(), k)).collect();
    w.projectiles = projectiles.into_iter().map(|p| (p.id.clone(), p)).collect();
    w.map = map;
    w.armour = armour;
    let mut s = State::with_world(Arc::new(w), 1);
    for army in [1, 2] {
        s.add_army(Army(army), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
    }
    s
}

fn plain(units: Vec<UnitKind>, projectiles: Vec<ProjectileKind>) -> State {
    arena(units, projectiles, flat_map(64), BTreeMap::new())
}

fn put(s: &mut State, army: u8, blueprint: &str, x: f64, z: f64, health: i64) -> EntityId {
    s.spawn(Army(army), blueprint, pos(x, 0.0, z), q(health))
}

fn health(s: &State, id: EntityId) -> Q {
    s.entities.get(&id).expect("the entity exists").health
}

fn move_to(s: &mut State, id: EntityId, x: f64, z: f64) {
    let e = s.entities.get_mut(&id).expect("the entity exists");
    e.pos = pos(x, 0.0, z);
}

fn targets(s: &State, id: EntityId) -> Vec<Option<EntityId>> {
    s.entities
        .get(&id)
        .expect("the entity exists")
        .weapons
        .iter()
        .map(|w| w.target)
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn proj(
    id: u32,
    kind: Option<&str>,
    at: Pos,
    vel: [f64; 3],
    target: Option<EntityId>,
    aim: Pos,
    damage: i64,
    radius: f64,
) -> Projectile {
    Projectile {
        id: ProjectileId(id),
        army: Army(1),
        source: EntityId(0),
        kind: kind.map(str::to_owned),
        pos: at,
        vel,
        target,
        aim,
        damage: q(damage),
        damage_radius: radius,
        damage_type: "Normal".to_owned(),
        lifetime: None,
    }
}

fn launch(s: &mut State, p: Projectile) {
    s.next_projectile_id = ProjectileId(s.next_projectile_id.0.max(p.id.0 + 1));
    s.projectiles.insert(p.id, p);
}

fn dummy() -> UnitKind {
    kind("dummy", &["STRUCTURE"])
}

/// SIM-075: the nearest allowed enemy in range is taken, ties go to the
/// lowest id, a held target is dropped out of range, and a bad Attack is
/// popped and counted by the weapons phase.
#[test]
fn sim_weapon_acquires_nearest_enemy_in_range_by_id() {
    let mut weapon = gun("Direct Fire", 20.0, 0.1, 0);
    weapon.min_radius = Some(3.0);
    let units = vec![armed("turret", vec![weapon]), dummy()];
    let mut s = plain(units.clone(), vec![]);
    let turret = put(&mut s, 1, "turret", 10.5, 10.5, 100);
    let a = put(&mut s, 2, "dummy", 25.5, 10.5, 100); // 15
    let b = put(&mut s, 2, "dummy", 22.5, 10.5, 100); // 12
    let _beyond = put(&mut s, 2, "dummy", 35.5, 10.5, 100); // 25
    let _inside = put(&mut s, 2, "dummy", 12.5, 10.5, 100); // 2, inside min
    let air = put(&mut s, 2, "dummy", 15.5, 10.5, 100); // 5, forbidden layer
    s.entities.get_mut(&air).expect("air").layer = Layer::Air;
    let _own = put(&mut s, 1, "dummy", 14.5, 10.5, 100); // 4, own army
    step(&mut s, &[]);
    assert_eq!(targets(&s, turret), vec![Some(b)], "the nearest allowed");
    move_to(&mut s, b, 40.5, 10.5);
    step(&mut s, &[]);
    assert_eq!(targets(&s, turret), vec![Some(a)], "dropped, then the next");
    move_to(&mut s, a, 40.5, 10.5);
    step(&mut s, &[]);
    assert_eq!(targets(&s, turret), vec![None], "nothing left in range");

    // Two enemies at the same distance: the lower id, which is the one
    // placed along z, not the one along x.
    let mut s = plain(units.clone(), vec![]);
    let turret = put(&mut s, 1, "turret", 10.5, 10.5, 100);
    let low = put(&mut s, 2, "dummy", 10.5, 18.5, 100);
    let _high = put(&mut s, 2, "dummy", 18.5, 10.5, 100);
    step(&mut s, &[]);
    assert_eq!(targets(&s, turret), vec![Some(low)]);

    // A bad Attack is popped and counted by the weapons phase, whether the
    // holder can move or not.
    let mut s = plain(units, vec![]);
    let t1 = put(&mut s, 1, "turret", 10.5, 10.5, 100);
    let t2 = put(&mut s, 1, "turret", 12.5, 10.5, 100);
    let friend = put(&mut s, 1, "dummy", 14.5, 10.5, 100);
    let cmds = [
        order_cmd(
            0,
            0,
            t1,
            Order::Attack { target: friend },
            QueueMode::Replace,
        ),
        order_cmd(
            0,
            1,
            t2,
            Order::Attack {
                target: EntityId(999),
            },
            QueueMode::Replace,
        ),
    ];
    step(&mut s, &cmds);
    assert_eq!(s.counters.commands_applied, 2);
    assert_eq!(s.counters.commands_ignored, 2);
    for t in [t1, t2] {
        assert!(s.entities.get(&t).expect("turret").orders.is_empty());
    }
}

/// SIM-076: a shot every ceiling-of-ten-over-the-rate ticks, from the first
/// tick the weapon holds a target; inactive weapons never fire; a weapon
/// without a kind or a speed makes instant hits.
#[test]
fn sim_weapon_fires_at_its_rate_of_fire() {
    let mut slow = gun("Direct Fire", 50.0, 2.0, 0);
    slow.projectile_id = Some("shell".to_owned());
    let mut fast = slow.clone();
    fast.rate_of_fire = Some(10.0);
    let units = vec![
        armed("slow", vec![slow.clone()]),
        armed("fast", vec![fast]),
        dummy(),
    ];
    let shells = vec![shot_kind("shell", 10.0)];
    let mut s = plain(units.clone(), shells.clone());
    let shooter = put(&mut s, 1, "slow", 5.5, 10.5, 100);
    let target = put(&mut s, 2, "dummy", 45.5, 10.5, 1000);
    step(&mut s, &[]);
    assert_eq!(s.projectiles.len(), 1, "fired on the first tick");
    let first = s.projectiles.values().next().expect("a shell");
    near(first.pos.x, 6.5);
    for _ in 1..30 {
        step(&mut s, &[]);
    }
    assert_eq!(
        s.next_projectile_id,
        ProjectileId(7),
        "six shots in 30 ticks"
    );
    assert_eq!(s.projectiles.len(), 6);
    for (k, p) in s.projectiles.values().enumerate() {
        assert_eq!(p.id, ProjectileId(u32::try_from(k).expect("small") + 1));
        assert_eq!(
            (p.source, p.army, p.target),
            (shooter, Army(1), Some(target))
        );
        near(
            p.pos.x,
            5.5 + f64::from(30 - 5 * u32::try_from(k).expect("small")),
        );
        near(p.aim.x, 45.5);
    }
    assert_eq!(s.counters.projectile_impacts, 0);

    let mut s = plain(units.clone(), shells);
    put(&mut s, 1, "fast", 5.5, 10.5, 100);
    put(&mut s, 2, "dummy", 45.5, 10.5, 1000);
    for _ in 0..10 {
        step(&mut s, &[]);
    }
    assert_eq!(s.next_projectile_id, ProjectileId(11), "a shot every tick");

    // Weapons that must not fire.
    let norate = WeaponKind {
        rate_of_fire: None,
        ..gun("Direct Fire", 50.0, 2.0, 5)
    };
    let manual = WeaponKind {
        manual_fire: true,
        ..gun("Direct Fire", 50.0, 2.0, 5)
    };
    let gated = WeaponKind {
        enabled_by_enhancement: true,
        ..gun("Direct Fire", 50.0, 2.0, 5)
    };
    let inert = vec![
        norate,
        gun("Death", 50.0, 2.0, 5),
        gun("Kamikaze", 50.0, 2.0, 5),
        manual,
        gated,
    ];
    let mut s = plain(vec![armed("inert", inert), dummy()], vec![]);
    let shooter = put(&mut s, 1, "inert", 5.5, 10.5, 100);
    let target = put(&mut s, 2, "dummy", 15.5, 10.5, 1000);
    for _ in 0..20 {
        step(&mut s, &[]);
    }
    assert_eq!(s.next_projectile_id, ProjectileId(1));
    assert_eq!(s.counters.projectile_impacts, 0);
    assert_eq!(health(&s, target), q(1000));
    assert_eq!(targets(&s, shooter), vec![None; 5]);

    // No kind, an unknown kind, and a kind without a speed: instant hits.
    let mut none = gun("Direct Fire", 50.0, 10.0, 10);
    let mut unknown = none.clone();
    unknown.projectile_id = Some("nope".to_owned());
    let mut slowless = none.clone();
    slowless.projectile_id = Some("still".to_owned());
    none.projectile_id = None;
    let still = ProjectileKind {
        id: "still".to_owned(),
        ..ProjectileKind::default()
    };
    let mut s = plain(
        vec![armed("quick", vec![none, unknown, slowless]), dummy()],
        vec![still],
    );
    put(&mut s, 1, "quick", 5.5, 10.5, 100);
    let target = put(&mut s, 2, "dummy", 45.5, 10.5, 1000);
    step(&mut s, &[]);
    assert!(
        s.projectiles.is_empty(),
        "instant hits resolve the same tick"
    );
    assert_eq!(s.next_projectile_id, ProjectileId(4));
    assert_eq!(s.counters.projectile_impacts, 3);
    assert_eq!(health(&s, target), q(970));
}

fn stepped_arena(kinds: Vec<ProjectileKind>) -> (State, EntityId) {
    let mut s = plain(vec![dummy()], kinds);
    let target = put(&mut s, 2, "dummy", 35.5, 10.5, 1000);
    (s, target)
}

fn alive(s: &State, id: u32) -> Option<&Projectile> {
    s.projectiles.get(&ProjectileId(id))
}

/// SIM-077: straight, fast, falling, tracking, expiring, ground, water, and
/// instant projectiles.
#[test]
fn sim_projectile_flies_falls_tracks_and_impacts() {
    // Speed 10 toward a target 25 away impacts in the 25th phase.
    let (mut s, t) = stepped_arena(vec![shot_kind("bullet", 10.0)]);
    launch(
        &mut s,
        proj(
            1,
            Some("bullet"),
            pos(10.5, 0.0, 10.5),
            [10.0, 0.0, 0.0],
            Some(t),
            pos(35.5, 0.0, 10.5),
            7,
            0.0,
        ),
    );
    for k in 1..=24 {
        step(&mut s, &[]);
        near(alive(&s, 1).expect("in flight").pos.x, 10.5 + f64::from(k));
    }
    assert_eq!(s.counters.projectile_impacts, 0);
    step(&mut s, &[]);
    assert!(alive(&s, 1).is_none());
    assert_eq!(s.counters.projectile_impacts, 1);
    assert_eq!(health(&s, t), q(993));

    // Speed 25 toward a target 4 away: the step of 2.5 would jump over a
    // half-unit sphere, so the swept test impacts in the second phase.
    let mut s = plain(vec![dummy()], vec![shot_kind("bullet", 25.0)]);
    let t = put(&mut s, 2, "dummy", 14.5, 10.5, 1000);
    launch(
        &mut s,
        proj(
            1,
            Some("bullet"),
            pos(10.5, 0.0, 10.5),
            [25.0, 0.0, 0.0],
            Some(t),
            pos(14.5, 0.0, 10.5),
            7,
            0.0,
        ),
    );
    step(&mut s, &[]);
    let p = alive(&s, 1).expect("still flying");
    near(p.pos.x, 13.0);
    assert!((14.5 - p.pos.x) > 0.5, "never within the impact radius");
    step(&mut s, &[]);
    assert!(alive(&s, 1).is_none());
    assert_eq!(health(&s, t), q(993));

    // A falling shell from a weapon at a target 7 away on flat ground. The
    // test replays the recurrence: gravity on vy, then the swept test on the
    // distance before the move, then the move by the mean of the old and
    // new velocity.
    let mut arc = shot_kind("arc", 25.0);
    arc.use_gravity = true;
    let mut cannon = gun("Artillery", 30.0, 0.03, 5);
    cannon.projectile_id = Some("arc".to_owned());
    cannon.muzzle_velocity = Some(25.0);
    let mut s = plain(vec![armed("cannon", vec![cannon]), dummy()], vec![arc]);
    put(&mut s, 1, "cannon", 10.5, 10.5, 100);
    let t = put(&mut s, 2, "dummy", 17.5, 10.5, 1000);
    let theta = 0.5 * libm::asin(4.9 * 7.0 / (25.0 * 25.0));
    let (vx, mut vy) = (25.0 * libm::cos(theta), 25.0 * libm::sin(theta));
    let (mut x, mut y) = (10.5, 0.0);
    let mut expected = Vec::new();
    let hit = loop {
        let old_vy = vy;
        vy -= 4.9 * 0.1;
        let reach = libm::sqrt(vx * vx + vy * vy) * 0.1 + 0.5;
        let d = libm::sqrt((17.5 - x) * (17.5 - x) + y * y);
        if d <= reach {
            break expected.len() + 1;
        }
        x += vx * 0.1;
        y += (old_vy + vy) / 2.0 * 0.1;
        expected.push((x, y));
        assert!(expected.len() < 10, "the replay must reach the target");
    };
    for (k, (ex, ey)) in expected.iter().enumerate() {
        step(&mut s, &[]);
        let p = alive(&s, 1).unwrap_or_else(|| panic!("in flight at tick {k}"));
        near(p.pos.x, *ex);
        near(p.pos.y, *ey);
        assert!(p.pos.y >= 0.0, "the flat shot stays above the ground");
    }
    step(&mut s, &[]);
    assert!(alive(&s, 1).is_none(), "impact in phase {hit}");
    assert_eq!(s.counters.projectile_impacts, 1);
    assert_eq!(health(&s, t), q(995));

    // Tracking: 90 degrees a second turns 0.15708 radians a tick at a
    // constant speed; a dumb twin flies straight on and leaves the map.
    let mut homer = shot_kind("homer", 10.0);
    homer.track_target = true;
    homer.turn_rate = Some(90.0);
    let mut snap = shot_kind("snap", 10.0);
    snap.track_target = true;
    let (mut s, t) = stepped_arena(vec![homer, snap, shot_kind("dumb", 10.0)]);
    move_to(&mut s, t, 10.5, 30.5);
    launch(
        &mut s,
        proj(
            1,
            Some("homer"),
            pos(10.5, 0.0, 10.5),
            [10.0, 0.0, 0.0],
            Some(t),
            pos(35.5, 0.0, 10.5),
            7,
            0.0,
        ),
    );
    launch(
        &mut s,
        proj(
            2,
            Some("dumb"),
            pos(10.5, 0.0, 10.5),
            [10.0, 0.0, 0.0],
            Some(t),
            pos(35.5, 0.0, 10.5),
            7,
            0.0,
        ),
    );
    launch(
        &mut s,
        proj(
            3,
            Some("snap"),
            pos(10.5, 0.0, 10.5),
            [10.0, 0.0, 0.0],
            Some(t),
            pos(35.5, 0.0, 10.5),
            7,
            0.0,
        ),
    );
    let mut before = [10.0, 0.0, 0.0];
    for _ in 0..4 {
        step(&mut s, &[]);
        let v = alive(&s, 1).expect("homing").vel;
        let speed = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        near(speed, 10.0);
        let dot = (v[0] * before[0] + v[1] * before[1] + v[2] * before[2]) / 100.0;
        near(libm::acos(dot), 90.0 * core::f64::consts::PI / 180.0 * 0.1);
        before = v;
        assert_eq!(alive(&s, 2).expect("dumb").vel, [10.0, 0.0, 0.0]);
    }
    for _ in 0..80 {
        step(&mut s, &[]);
    }
    assert!(alive(&s, 1).is_none() && alive(&s, 2).is_none());
    assert_eq!(health(&s, t), q(1000 - 7 - 7), "the homer and the snap hit");

    let mut snap_only = shot_kind("snap", 10.0);
    snap_only.track_target = true;
    let (mut s, t) = stepped_arena(vec![snap_only]);
    move_to(&mut s, t, 10.5, 30.5);
    launch(
        &mut s,
        proj(
            1,
            Some("snap"),
            pos(10.5, 0.0, 10.5),
            [10.0, 0.0, 0.0],
            Some(t),
            pos(35.5, 0.0, 10.5),
            7,
            0.0,
        ),
    );
    step(&mut s, &[]);
    let v = alive(&s, 1).expect("snapping").vel;
    near(v[0], 0.0);
    near(v[2], 10.0);

    // A lifetime of 0.2 seconds ends in two ticks without an impact.
    let (mut s, t) = stepped_arena(vec![shot_kind("bullet", 1.0)]);
    let mut p = proj(
        1,
        Some("bullet"),
        pos(10.5, 0.0, 10.5),
        [1.0, 0.0, 0.0],
        Some(t),
        pos(35.5, 0.0, 10.5),
        7,
        0.0,
    );
    p.lifetime = Some(0.2);
    launch(&mut s, p);
    step(&mut s, &[]);
    near(alive(&s, 1).expect("alive").lifetime.expect("timed"), 0.1);
    step(&mut s, &[]);
    assert!(alive(&s, 1).is_none());
    assert_eq!(s.counters.projectile_impacts, 0);

    // A terrain step 5 high from corner column 15: the shell at height 2
    // lands in the fifth phase at x = 15.5 and hurts what is within 3.
    let samples: Vec<i16> = (0..=64)
        .flat_map(|_| (0..=64).map(|x| if x >= 15 { 640 } else { 0 }))
        .collect();
    let map = MapGrid::from_heightmap(64, 64, &samples, 0.007_812_5).expect("a stepped map");
    let mut s = arena(
        vec![dummy()],
        vec![shot_kind("bullet", 10.0)],
        map,
        BTreeMap::new(),
    );
    let near_one = put(&mut s, 2, "dummy", 16.5, 10.5, 100);
    let far_one = put(&mut s, 2, "dummy", 19.5, 10.5, 100);
    launch(
        &mut s,
        proj(
            1,
            Some("bullet"),
            pos(10.5, 2.0, 10.5),
            [10.0, 0.0, 0.0],
            None,
            pos(60.5, 2.0, 10.5),
            9,
            3.0,
        ),
    );
    for _ in 0..4 {
        step(&mut s, &[]);
    }
    assert_eq!(s.counters.projectile_impacts, 0);
    step(&mut s, &[]);
    assert_eq!(s.counters.projectile_impacts, 1);
    assert_eq!(health(&s, near_one), q(91));
    assert_eq!(health(&s, far_one), q(100));

    // Water: a shell below the surface that dies on water is removed; one
    // that does not carries on.
    let wet =
        MapGrid::from_heightmap_with_water(64, 64, &vec![0_i16; 65 * 65], 0.007_812_5, Some(4.0))
            .expect("a wet map");
    let mut drowns = shot_kind("drowns", 10.0);
    drowns.destroy_on_water = true;
    let mut s = arena(
        vec![dummy()],
        vec![drowns, shot_kind("dry", 10.0)],
        wet,
        BTreeMap::new(),
    );
    for (id, name) in [(1, "drowns"), (2, "dry")] {
        launch(
            &mut s,
            proj(
                id,
                Some(name),
                pos(10.5, 2.0, 10.5),
                [10.0, 0.0, 0.0],
                None,
                pos(60.5, 2.0, 10.5),
                9,
                0.0,
            ),
        );
    }
    step(&mut s, &[]);
    assert!(alive(&s, 1).is_none() && alive(&s, 2).is_some());
    assert_eq!(s.counters.projectile_impacts, 0);

    // An instant hit lands the same tick at the target's current position,
    // not at the aim it was fired at.
    let mut s = plain(vec![dummy()], vec![]);
    let t = put(&mut s, 2, "dummy", 20.5, 20.5, 100);
    let by_target = put(&mut s, 2, "dummy", 21.5, 20.5, 100);
    let by_aim = put(&mut s, 2, "dummy", 50.5, 21.5, 100);
    launch(
        &mut s,
        proj(
            1,
            None,
            pos(10.5, 0.0, 10.5),
            [0.0; 3],
            Some(t),
            pos(50.5, 0.0, 20.5),
            10,
            2.0,
        ),
    );
    step(&mut s, &[]);
    assert_eq!(
        [health(&s, t), health(&s, by_target), health(&s, by_aim)],
        [q(90), q(90), q(100)]
    );
}

/// SIM-078: damage times the armour multiplier, uniform inside the radius to
/// enemies only, a ground impact through the radius alone.
#[test]
fn sim_damage_applies_armour_and_radius() {
    let table = vec![
        (
            "Commander".to_owned(),
            vec![
                ("Normal".to_owned(), 1.0),
                ("Overcharge".to_owned(), 0.033_333),
            ],
        ),
        (
            "Structure".to_owned(),
            vec![("Overcharge".to_owned(), 0.066_666)],
        ),
    ];
    let built =
        World::from_parts_with_armour(&[], &[], flat_map(64), Vec::new(), "t".to_owned(), &table)
            .expect("a world");
    let mut units = Vec::new();
    for (id, armour) in [
        ("cmdr", "Commander"),
        ("bldg", "Structure"),
        ("plain", "Normal"),
    ] {
        let mut k = kind(id, &["STRUCTURE"]);
        k.armor_type = armour.to_owned();
        units.push(k);
    }
    let mut w = built;
    w.units = units.into_iter().map(|k| (k.id.clone(), k)).collect();
    let world = Arc::new(w);
    let fresh = || {
        let mut s = State::with_world(Arc::clone(&world), 1);
        for army in [1, 2] {
            s.add_army(Army(army), Q::ZERO, Q::ZERO, Q::ZERO, Q::ZERO);
        }
        s
    };
    // The multiplier by SIM-050's rule, the product by integer arithmetic
    // on the raw bits (the fixed-point product floors).
    let times = |damage: i64, decimal: f64| {
        let m = Q::checked_from_num(decimal).expect("a representable decimal");
        let raw = ((i128::from(damage) << 32) * i128::from(m.to_bits())) >> 32;
        Q::from_bits(i64::try_from(raw).expect("fits"))
    };
    let hit = |s: &mut State, t: EntityId, ty: &str| {
        let mut p = proj(
            1,
            None,
            pos(1.5, 0.0, 1.5),
            [0.0; 3],
            Some(t),
            pos(1.5, 0.0, 1.5),
            100,
            0.0,
        );
        p.damage_type = ty.to_owned();
        s.projectiles.clear();
        launch(s, p);
        step(s, &[]);
    };
    let mut s = fresh();
    let cmdr = put(&mut s, 2, "cmdr", 30.5, 30.5, 1000);
    let bldg = put(&mut s, 2, "bldg", 32.5, 30.5, 1000);
    let plain_id = put(&mut s, 2, "plain", 34.5, 30.5, 1000);
    hit(&mut s, cmdr, "Overcharge");
    assert_eq!(health(&s, cmdr), q(1000) - times(100, 0.033_333));
    hit(&mut s, bldg, "Overcharge");
    assert_eq!(health(&s, bldg), q(1000) - times(100, 0.066_666));
    hit(&mut s, cmdr, "Normal");
    assert_eq!(health(&s, cmdr), q(1000) - times(100, 0.033_333) - q(100));
    hit(&mut s, plain_id, "Overcharge");
    assert_eq!(health(&s, plain_id), q(900), "an armour outside the table");
    hit(&mut s, cmdr, "EMP");
    assert_eq!(
        health(&s, cmdr),
        q(1000) - times(100, 0.033_333) - q(200),
        "an unknown damage type"
    );
    assert_eq!(s.counters.projectile_impacts, 5);

    // A radius of 3 around the entity hit.
    let mut s = fresh();
    let on = put(&mut s, 2, "plain", 20.5, 20.5, 1000);
    let at_two = put(&mut s, 2, "plain", 20.5, 22.5, 1000);
    let at_three = put(&mut s, 2, "plain", 23.5, 20.5, 1000);
    let at_far = put(&mut s, 2, "plain", 20.5, 24.0, 1000);
    let own = put(&mut s, 1, "plain", 21.5, 20.5, 1000);
    launch(
        &mut s,
        proj(
            1,
            None,
            pos(1.5, 0.0, 1.5),
            [0.0; 3],
            Some(on),
            pos(20.5, 0.0, 20.5),
            100,
            3.0,
        ),
    );
    step(&mut s, &[]);
    for t in [on, at_two, at_three] {
        assert_eq!(health(&s, t), q(900));
    }
    assert_eq!((health(&s, at_far), health(&s, own)), (q(1000), q(1000)));
    assert_eq!(s.counters.projectile_impacts, 1);

    // A ground impact away from the target hurts through the radius alone.
    let mut s = fresh();
    let stale = put(&mut s, 2, "plain", 5.5, 5.5, 1000);
    let by_ground = put(&mut s, 2, "plain", 41.5, 40.5, 1000);
    launch(
        &mut s,
        proj(
            1,
            None,
            pos(1.5, 0.0, 1.5),
            [0.0; 3],
            None,
            pos(40.5, 0.0, 40.5),
            100,
            2.0,
        ),
    );
    step(&mut s, &[]);
    assert_eq!(
        (health(&s, stale), health(&s, by_ground)),
        (q(1000), q(900))
    );
}

/// SIM-079: a tenth of the rate each tick, to the maximum, for a complete
/// entity above zero health only.
#[test]
fn sim_regen_heals_per_tick_to_max() {
    let mut healer = dummy();
    healer.id = "healer".to_owned();
    healer.regen_rate = Some(q(10));
    let mut slow = healer.clone();
    slow.id = "slow".to_owned();
    slow.regen_rate = Some(Q::from_num(0.5));
    let mut site = healer.clone();
    site.id = "site".to_owned();
    site.build_time = Some(q(64));
    site.max_health = q(128);
    let mut s = plain(vec![healer, slow, site], vec![]);
    let a = put(&mut s, 1, "healer", 5.5, 5.5, 100);
    s.entities.get_mut(&a).expect("a").health = q(50);
    let b = put(&mut s, 1, "healer", 7.5, 5.5, 100);
    let c = put(&mut s, 1, "healer", 9.5, 5.5, 100);
    s.entities.get_mut(&c).expect("c").health = Q::from_num(99.5);
    let d = put(&mut s, 1, "slow", 11.5, 5.5, 100);
    s.entities.get_mut(&d).expect("d").health = q(50);
    let site = s
        .spawn_under_construction(Army(1), "site", pos(13.5, 0.0, 5.5))
        .expect("a site");
    step(&mut s, &[]);
    assert_eq!(health(&s, a), q(51));
    assert_eq!(health(&s, b), q(100), "at the maximum, nothing");
    assert_eq!(health(&s, c), q(100), "capped");
    let tenth = Q::from_bits(Q::from_num(0.5).to_bits() / 10);
    assert_eq!(health(&s, d), q(50) + tenth);
    assert_eq!(
        health(&s, site),
        Q::ZERO,
        "an unfinished entity gains nothing"
    );
    for _ in 0..4 {
        step(&mut s, &[]);
    }
    assert_eq!(health(&s, a), q(55));

    // Regeneration never resurrects: at zero the entity is removed.
    let dying = put(&mut s, 1, "healer", 15.5, 5.5, 100);
    s.entities.get_mut(&dying).expect("dying").health = Q::ZERO;
    step(&mut s, &[]);
    assert!(!s.entities.contains_key(&dying));
    assert_eq!(s.counters.deaths, 1);
}

/// SIM-080: removal at health at or below 0, counted, orders gone, an
/// Attack on the dead popped, shells in flight keep their aim, and an
/// unfinished entity dies only of lost work.
#[test]
fn sim_death_removes_the_entity_and_counts() {
    let mut site = dummy();
    site.id = "site".to_owned();
    site.build_time = Some(q(64));
    site.max_health = q(128);
    let turret = armed("turret", vec![gun("Direct Fire", 40.0, 0.1, 0)]);
    let mut s = plain(vec![dummy(), site, turret], vec![shot_kind("bullet", 10.0)]);
    let zero = put(&mut s, 2, "dummy", 40.5, 40.5, 100);
    let below = put(&mut s, 2, "dummy", 42.5, 40.5, 100);
    let fine = put(&mut s, 2, "dummy", 44.5, 40.5, 100);
    s.entities.get_mut(&zero).expect("zero").health = Q::ZERO;
    s.entities
        .get_mut(&zero)
        .expect("zero")
        .orders
        .push_back(Order::Stop);
    s.entities.get_mut(&below).expect("below").health = q(-5);
    step(&mut s, &[]);
    assert!(!s.entities.contains_key(&zero) && !s.entities.contains_key(&below));
    assert!(s.entities.contains_key(&fine));
    assert_eq!(s.counters.deaths, 2);

    // An Attack on the dead is popped, and not counted as ignored.
    let victim = put(&mut s, 2, "dummy", 30.5, 10.5, 100);
    let attacker = put(&mut s, 1, "turret", 10.5, 10.5, 100);
    s.entities.get_mut(&victim).expect("victim").health = Q::ZERO;
    let ignored = s.counters.commands_ignored;
    step(
        &mut s,
        &[order_cmd(
            1,
            0,
            attacker,
            Order::Attack { target: victim },
            QueueMode::Replace,
        )],
    );
    assert!(!s.entities.contains_key(&victim));
    assert!(
        s.entities
            .get(&attacker)
            .expect("attacker")
            .orders
            .is_empty()
    );
    assert_eq!(s.counters.commands_ignored, ignored);

    // A shell aimed at a dead entity flies on to the aim and lands there.
    let mut s = plain(vec![dummy()], vec![shot_kind("bullet", 10.0)]);
    let victim = put(&mut s, 2, "dummy", 30.5, 10.5, 100);
    let by_aim = put(&mut s, 2, "dummy", 31.5, 10.5, 100);
    s.entities.get_mut(&victim).expect("victim").health = Q::ZERO;
    launch(
        &mut s,
        proj(
            1,
            Some("bullet"),
            pos(10.5, 0.0, 10.5),
            [10.0, 0.0, 0.0],
            Some(victim),
            pos(30.5, 0.0, 10.5),
            5,
            2.0,
        ),
    );
    for _ in 0..19 {
        step(&mut s, &[]);
    }
    assert_eq!(s.counters.projectile_impacts, 0);
    assert!(!s.entities.contains_key(&victim));
    step(&mut s, &[]);
    assert_eq!(s.counters.projectile_impacts, 1);
    assert_eq!(health(&s, by_aim), q(95));

    // A site: at work 0 it is not removed; a hit for less than its work
    // keeps health at max times progress; a hit for more removes it.
    let mut site = dummy();
    site.id = "site".to_owned();
    site.build_time = Some(q(64));
    site.max_health = q(128);
    let mut s = plain(vec![site], vec![]);
    let id = s
        .spawn_under_construction(Army(2), "site", pos(20.5, 0.0, 20.5))
        .expect("a site");
    step(&mut s, &[]);
    assert!(s.entities.contains_key(&id), "work 0 alone does not kill");
    {
        let e = s.entities.get_mut(&id).expect("site");
        e.built = q(32);
        e.health = q(64);
    }
    launch(
        &mut s,
        proj(
            1,
            None,
            pos(1.5, 0.0, 1.5),
            [0.0; 3],
            Some(id),
            pos(20.5, 0.0, 20.5),
            32,
            0.0,
        ),
    );
    step(&mut s, &[]);
    let e = s.entities.get(&id).expect("site survives");
    assert_eq!((e.built, e.health), (q(16), q(32)));
    launch(
        &mut s,
        proj(
            2,
            None,
            pos(1.5, 0.0, 1.5),
            [0.0; 3],
            Some(id),
            pos(20.5, 0.0, 20.5),
            256,
            0.0,
        ),
    );
    step(&mut s, &[]);
    assert!(!s.entities.contains_key(&id));
    assert_eq!(s.counters.deaths, 1);
}

/// SIM-081: an Attack approaches by the follow rule to the longest weapon
/// range, fires at the clock, and is popped when the target dies; an
/// unarmed unit's Attack is dropped and counted.
#[test]
fn sim_attack_order_approaches_and_fires() {
    let mut tank = armed("tank", vec![gun("Direct Fire", 10.0, 1.0, 40)]);
    tank.motion = MotionType::Land;
    tank.max_speed = Some(3.0);
    let mut grunt = kind("grunt", &["MOBILE"]);
    grunt.motion = MotionType::Land;
    grunt.max_speed = Some(3.0);
    let mut s = plain(vec![tank, grunt, dummy()], vec![]);
    let tank_id = put(&mut s, 1, "tank", 5.5, 5.5, 100);
    let site = put(&mut s, 2, "dummy", 35.5, 5.5, 100);
    let mut shots = Vec::new();
    let mut gap_before = f64::MAX;
    let mut at_shot = Vec::new();
    for tick in 0..200_u64 {
        let cmds = if tick == 0 {
            vec![order_cmd(
                0,
                0,
                tank_id,
                Order::Attack { target: site },
                QueueMode::Replace,
            )]
        } else {
            Vec::new()
        };
        let impacts = s.counters.projectile_impacts;
        step(&mut s, &cmds);
        let x = s.entities.get(&tank_id).expect("tank").pos.x;
        if s.counters.projectile_impacts > impacts {
            shots.push(tick);
            at_shot.push((gap_before, 35.5 - x));
        }
        gap_before = 35.5 - x;
        if !s.entities.contains_key(&site) {
            break;
        }
    }
    let first = *shots.first().expect("it fired");
    let (before, during) = at_shot[0];
    assert!(
        before > 10.0 && during <= 10.0,
        "fired on the first tick in range"
    );
    assert_eq!(
        shots,
        vec![first, first + 10, first + 20],
        "a shot every ten ticks"
    );
    assert!(!s.entities.contains_key(&site));
    assert!(s.entities.get(&tank_id).expect("tank").orders.is_empty());
    assert_eq!(s.counters.deaths, 1);

    // An unarmed unit drops its Attack and stays put.
    let mut s = plain(
        vec![
            {
                let mut g = kind("grunt", &["MOBILE"]);
                g.motion = MotionType::Land;
                g.max_speed = Some(3.0);
                g
            },
            dummy(),
        ],
        vec![],
    );
    let g = put(&mut s, 1, "grunt", 5.5, 5.5, 100);
    let site = put(&mut s, 2, "dummy", 35.5, 5.5, 100);
    step(
        &mut s,
        &[order_cmd(
            0,
            0,
            g,
            Order::Attack { target: site },
            QueueMode::Replace,
        )],
    );
    assert!(s.entities.get(&g).expect("grunt").orders.is_empty());
    assert_eq!(s.counters.commands_ignored, 1);
    near(s.entities.get(&g).expect("grunt").pos.x, 5.5);
}

/// The armour table: multipliers converted by SIM-050's rule, absent keys
/// read as 1, and the table enters the catalog digest.
#[test]
fn sim_armour_table_from_parsed_values() {
    let table = vec![
        (
            "Commander".to_owned(),
            vec![
                ("Normal".to_owned(), 1.0),
                ("Overcharge".to_owned(), 0.033_333),
            ],
        ),
        ("Structure".to_owned(), vec![("Deathnuke".to_owned(), 0.01)]),
    ];
    let with =
        World::from_parts_with_armour(&[], &[], flat_map(4), Vec::new(), "t".to_owned(), &table)
            .expect("a world");
    let without =
        World::from_parts(&[], &[], flat_map(4), Vec::new(), "t".to_owned()).expect("a world");
    let oc = Q::checked_from_num(0.033_333).expect("representable");
    assert_eq!(
        with.armour
            .get("Commander")
            .and_then(|r| r.get("Overcharge"))
            .copied(),
        Some(oc)
    );
    assert_eq!(with.armour_multiplier("Commander", "Overcharge"), oc);
    assert_eq!(
        with.armour_multiplier("Structure", "Deathnuke"),
        Q::checked_from_num(0.01).expect("ok")
    );
    assert_eq!(with.armour_multiplier("Commander", "EMP"), Q::ONE);
    assert_eq!(with.armour_multiplier("Nope", "Normal"), Q::ONE);
    assert!(without.armour.is_empty());
    assert_ne!(with.catalog_digest, without.catalog_digest);
}

/// SIM-075: no weapon takes a target of its own army, not even the head
/// Attack's. A turret ordered to attack one own-army unit, then another,
/// launches nothing, hurts nothing, and has both orders popped and counted.
#[test]
fn sim_weapon_never_takes_an_own_army_target() {
    let units = vec![
        armed("turret", vec![gun("Direct Fire", 20.0, 1.0, 10)]),
        dummy(),
    ];
    let mut s = plain(units, vec![]);
    let turret = put(&mut s, 1, "turret", 10.5, 10.5, 100);
    let first = put(&mut s, 1, "dummy", 14.5, 10.5, 100);
    let second = put(&mut s, 1, "dummy", 16.5, 10.5, 100);
    let cmds = [
        order_cmd(
            0,
            0,
            turret,
            Order::Attack { target: first },
            QueueMode::Replace,
        ),
        order_cmd(
            0,
            1,
            turret,
            Order::Attack { target: second },
            QueueMode::Append,
        ),
    ];
    step(&mut s, &cmds);
    step(&mut s, &[]);
    assert!(s.projectiles.is_empty());
    assert_eq!(s.counters.projectile_impacts, 0);
    assert_eq!((health(&s, first), health(&s, second)), (q(100), q(100)));
    assert_eq!(targets(&s, turret), vec![None]);
    assert!(s.entities.get(&turret).expect("turret").orders.is_empty());
    assert_eq!(s.counters.commands_applied, 2);
    assert_eq!(s.counters.commands_ignored, 2);
}

fn turret_of(weapon: WeaponKind) -> Vec<UnitKind> {
    vec![armed("turret", vec![weapon]), dummy()]
}

/// SIM-075: both radii are inclusive; a target exactly at the maximum and
/// one exactly at the minimum are taken.
#[test]
fn sim_weapon_radii_are_inclusive() {
    let mut weapon = gun("Direct Fire", 10.0, 1.0, 0);
    weapon.min_radius = Some(3.0);
    let units = turret_of(weapon);
    // 20.5 - 10.5 and 13.5 - 10.5 are exact in binary64.
    for (x, what) in [(20.5, "at the maximum"), (13.5, "at the minimum")] {
        let mut s = plain(units.clone(), vec![]);
        let turret = put(&mut s, 1, "turret", 10.5, 10.5, 100);
        let target = put(&mut s, 2, "dummy", x, 10.5, 100);
        step(&mut s, &[]);
        assert_eq!(targets(&s, turret), vec![Some(target)], "{what}");
    }
    for (x, what) in [(20.6, "past the maximum"), (13.4, "inside the minimum")] {
        let mut s = plain(units.clone(), vec![]);
        let turret = put(&mut s, 1, "turret", 10.5, 10.5, 100);
        put(&mut s, 2, "dummy", x, 10.5, 100);
        step(&mut s, &[]);
        assert_eq!(targets(&s, turret), vec![None], "{what}");
    }
}

/// SIM-075: a held target stays held while it remains allowed, even when a
/// nearer enemy appears.
#[test]
fn sim_weapon_keeps_a_held_target() {
    let mut s = plain(turret_of(gun("Direct Fire", 20.0, 0.1, 0)), vec![]);
    let turret = put(&mut s, 1, "turret", 10.5, 10.5, 100);
    let far = put(&mut s, 2, "dummy", 25.5, 10.5, 100);
    step(&mut s, &[]);
    assert_eq!(targets(&s, turret), vec![Some(far)]);
    let _near = put(&mut s, 2, "dummy", 15.5, 10.5, 100);
    step(&mut s, &[]);
    assert_eq!(targets(&s, turret), vec![Some(far)], "held over a nearer");
}

/// SIM-075: the head Attack's target is taken ahead of a nearer enemy.
#[test]
fn sim_weapon_prefers_the_attack_target() {
    let mut s = plain(turret_of(gun("Direct Fire", 20.0, 0.1, 0)), vec![]);
    let turret = put(&mut s, 1, "turret", 10.5, 10.5, 100);
    let far = put(&mut s, 2, "dummy", 25.5, 10.5, 100);
    let _near = put(&mut s, 2, "dummy", 15.5, 10.5, 100);
    let cmd = order_cmd(
        0,
        0,
        turret,
        Order::Attack { target: far },
        QueueMode::Replace,
    );
    step(&mut s, &[cmd]);
    assert_eq!(targets(&s, turret), vec![Some(far)]);
}

/// SIM-076: the cooldown is the ceiling of ten over the rate.
#[test]
fn sim_cooldown_is_the_ceiling_of_ten_over_the_rate() {
    assert_eq!(supercom_sim::cooldown_ticks(3.0), 4, "3.33 rounds up");
    assert_eq!(supercom_sim::cooldown_ticks(0.03), 334, "333.3 rounds up");
    assert_eq!(supercom_sim::cooldown_ticks(2.0), 5, "exact stays exact");
    assert_eq!(supercom_sim::cooldown_ticks(100.0), 1, "at least 1");
}

/// SIM-077: a falling shell aimed beyond its speed squared over 4.9 on
/// flat ground launches at 45 degrees (the arcsine argument is clamped).
#[test]
fn sim_falling_shot_beyond_reach_launches_at_45_degrees() {
    let mut arc = shot_kind("arc", 10.0);
    arc.use_gravity = true;
    let mut cannon = gun("Artillery", 40.0, 0.03, 5);
    cannon.projectile_id = Some("arc".to_owned());
    cannon.muzzle_velocity = Some(10.0);
    let mut s = plain(turret_of(cannon), vec![arc]);
    put(&mut s, 1, "turret", 10.5, 10.5, 100);
    put(&mut s, 2, "dummy", 40.5, 10.5, 1000);
    step(&mut s, &[]);
    let p = s.projectiles.values().next().expect("fired");
    // Reach is 100 / 4.9 = 20.4 and the target is 30 away; the projectiles
    // phase has already moved the shell once, but its x velocity is the
    // launch's, and gravity has lowered only vy.
    let half = 10.0 * (0.5_f64).sqrt();
    near(p.vel[0], half);
    near(p.vel[1], half - 4.9 * 0.1);
    near(p.vel[2], 0.0);
}

/// SIM-077: an aim straight above or below the firer, at planar distance 0,
/// is an instant hit, not a projectile.
#[test]
fn sim_aim_at_planar_distance_zero_hits_at_once() {
    let mut weapon = gun("Direct Fire", 20.0, 1.0, 10);
    weapon.projectile_id = Some("shell".to_owned());
    weapon.muzzle_velocity = Some(10.0);
    let mut s = plain(turret_of(weapon), vec![shot_kind("shell", 10.0)]);
    put(&mut s, 1, "turret", 10.5, 10.5, 100);
    let target = put(&mut s, 2, "dummy", 10.5, 10.5, 100);
    s.entities.get_mut(&target).expect("target").pos.y = 5.0;
    step(&mut s, &[]);
    assert!(s.projectiles.is_empty(), "no projectile");
    assert_eq!(s.counters.projectile_impacts, 1);
    assert_eq!(health(&s, target), q(90));
}

/// SIM-077: acceleration raises the speed by acceleration times 0.1 a tick
/// up to the maximum, which a step of 10 * 0.1 from 4 overshoots, and
/// holds it there.
#[test]
fn sim_projectile_accelerates_to_its_maximum_speed() {
    let mut rocket = shot_kind("rocket", 2.0);
    rocket.acceleration = Some(10.0);
    rocket.max_speed = Some(4.5);
    let mut s = plain(vec![dummy()], vec![rocket]);
    launch(
        &mut s,
        proj(
            1,
            Some("rocket"),
            pos(10.5, 0.0, 10.5),
            [2.0, 0.0, 0.0],
            None,
            pos(60.5, 0.0, 10.5),
            1,
            0.0,
        ),
    );
    let mut x = 10.5;
    let mut before = 2.0;
    for speed in [3.0, 4.0, 4.5, 4.5, 4.5] {
        step(&mut s, &[]);
        let p = alive(&s, 1).expect("in flight");
        near(p.vel[0], speed);
        x += (before + speed) / 2.0 * 0.1;
        near(p.pos.x, x);
        before = speed;
    }
}

/// SIM-077: a shot that passes the target at a distance between the step
/// and the step plus the impact radius lands on it.
#[test]
fn sim_projectile_impacts_within_the_impact_radius() {
    let (mut s, t) = stepped_arena(vec![shot_kind("bullet", 10.0)]);
    // The shell flies along z = 10.5 past a target 1.2 off its line: the
    // step is 1.0, the least distance 1.2, the reach 1.5.
    move_to(&mut s, t, 20.5, 11.7);
    launch(
        &mut s,
        proj(
            1,
            Some("bullet"),
            pos(10.5, 0.0, 10.5),
            [10.0, 0.0, 0.0],
            Some(t),
            pos(20.5, 0.0, 11.7),
            7,
            0.0,
        ),
    );
    for _ in 0..20 {
        step(&mut s, &[]);
    }
    assert_eq!(s.counters.projectile_impacts, 1);
    assert_eq!(health(&s, t), q(993));
}

/// SIM-078: an unfinished entity is marked for death by positive damage
/// only; a zero-damage hit at work 0 leaves it standing.
#[test]
fn sim_zero_damage_does_not_kill_a_site() {
    let mut site = dummy();
    site.id = "site".to_owned();
    site.build_time = Some(q(64));
    site.max_health = q(128);
    let mut s = plain(vec![site], vec![]);
    let id = s
        .spawn_under_construction(Army(2), "site", pos(20.5, 0.0, 20.5))
        .expect("a site");
    launch(
        &mut s,
        proj(
            1,
            None,
            pos(1.5, 0.0, 1.5),
            [0.0; 3],
            Some(id),
            pos(20.5, 0.0, 20.5),
            0,
            0.0,
        ),
    );
    step(&mut s, &[]);
    assert_eq!(s.counters.projectile_impacts, 1);
    assert!(s.entities.contains_key(&id), "zero damage does not kill");
    assert_eq!(s.counters.deaths, 0);
}

/// SIM-079: regeneration heals complete entities only; a site built partway
/// and hurt gains nothing.
#[test]
fn sim_regen_skips_a_part_built_site() {
    let mut site = dummy();
    site.id = "site".to_owned();
    site.build_time = Some(q(64));
    site.max_health = q(128);
    site.regen_rate = Some(q(10));
    let mut s = plain(vec![site], vec![]);
    let id = s
        .spawn_under_construction(Army(1), "site", pos(20.5, 0.0, 20.5))
        .expect("a site");
    {
        let e = s.entities.get_mut(&id).expect("site");
        e.built = q(32);
        e.health = q(64);
    }
    step(&mut s, &[]);
    assert_eq!(health(&s, id), q(64));
}

/// SIM-081: a mobile unit's Attack on an own-army unit moves nothing and is
/// popped and counted.
#[test]
fn sim_attack_on_own_army_does_not_move_the_attacker() {
    let mut tank = armed("tank", vec![gun("Direct Fire", 10.0, 1.0, 40)]);
    tank.motion = MotionType::Land;
    tank.max_speed = Some(3.0);
    let mut s = plain(vec![tank, dummy()], vec![]);
    let tank_id = put(&mut s, 1, "tank", 5.5, 5.5, 100);
    let friend = put(&mut s, 1, "dummy", 35.5, 5.5, 100);
    let cmd = order_cmd(
        0,
        0,
        tank_id,
        Order::Attack { target: friend },
        QueueMode::Replace,
    );
    step(&mut s, &[cmd]);
    for _ in 0..4 {
        step(&mut s, &[]);
    }
    let e = s.entities.get(&tank_id).expect("tank");
    near(e.pos.x, 5.5);
    near(e.pos.z, 5.5);
    assert!(e.orders.is_empty());
    assert_eq!(s.counters.commands_ignored, 1);
}

/// SIM-081: an attacker closes to its weapon range and stops there, short
/// of the guard's follow range.
#[test]
fn sim_attacker_stops_at_its_weapon_range() {
    let mut tank = armed("tank", vec![gun("Direct Fire", 10.0, 1.0, 0)]);
    tank.motion = MotionType::Land;
    tank.max_speed = Some(3.0);
    let mut s = plain(vec![tank, dummy()], vec![]);
    let tank_id = put(&mut s, 1, "tank", 5.5, 5.5, 100);
    let enemy = put(&mut s, 2, "dummy", 35.5, 5.5, 100);
    let cmd = order_cmd(
        0,
        0,
        tank_id,
        Order::Attack { target: enemy },
        QueueMode::Replace,
    );
    step(&mut s, &[cmd]);
    for _ in 0..119 {
        step(&mut s, &[]);
    }
    let gap = distance_xz(
        &s.entities.get(&tank_id).expect("tank").pos,
        &pos(35.5, 0.0, 5.5),
    );
    assert!(gap <= 10.0, "within the weapon range: {gap}");
    assert!(
        gap > supercom_sim::FOLLOW_RANGE,
        "and no closer than the follow range: {gap}"
    );
}
