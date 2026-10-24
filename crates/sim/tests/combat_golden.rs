//! A pinned digest of the combat phases (docs/specs/sim.md, SIM-075 to
//! SIM-080; docs/specs/determinism.md): one synthetic scenario runs a
//! falling shell with a damage radius against an armoured target, a
//! tracking, accelerating missile that kills a weak one, and the firing
//! clocks of both, and the final digest of the chain is a literal computed
//! once and written down, so a second host that runs the same tests
//! compares its binary64 flight with this one's. All synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use supercom_sim::{
    Army, Digest, LayerSet, Order, ProjectileKind, Q, QueueMode, State, UnitKind, WeaponKind, World,
};
use support::{flat_map, kind, order_cmd, pos, q, step};

/// The final digest of the scenario, computed once on `x86_64` from a run of
/// the scenario below and written here.
const GOLDEN: Digest = [
    101, 174, 100, 65, 153, 69, 57, 248, 209, 98, 51, 174, 227, 116, 77, 104, 97, 51, 48, 48, 120,
    237, 208, 197, 207, 96, 214, 250, 140, 221, 231, 221,
];

const TICKS: u64 = 150;

fn gun(category: &str, projectile: &str, speed: f64, rate: f64, damage: i64) -> WeaponKind {
    WeaponKind {
        category: category.to_owned(),
        damage: q(damage),
        damage_type: "Normal".to_owned(),
        rate_of_fire: Some(rate),
        max_radius: Some(60.0),
        muzzle_velocity: Some(speed),
        projectile_id: Some(projectile.to_owned()),
        damage_radius: Some(3.0),
        target_layers: LayerSet::from_caps("Land").expect("caps"),
        ..WeaponKind::default()
    }
}

fn world() -> Arc<World> {
    let shell = ProjectileKind {
        id: "shell".to_owned(),
        initial_speed: Some(18.0),
        use_gravity: true,
        ..ProjectileKind::default()
    };
    let missile = ProjectileKind {
        id: "missile".to_owned(),
        initial_speed: Some(4.0),
        acceleration: Some(6.0),
        max_speed: Some(14.0),
        track_target: true,
        turn_rate: Some(120.0),
        lifetime: Some(12.0),
        ..ProjectileKind::default()
    };
    let mut howitzer: UnitKind = kind("howitzer", &["STRUCTURE"]);
    howitzer.weapons = vec![gun("Artillery", "shell", 18.0, 0.2, 30)];
    let mut launcher: UnitKind = kind("launcher", &["STRUCTURE"]);
    launcher.weapons = vec![gun("Missile", "missile", 4.0, 0.5, 45)];
    let mut bunker: UnitKind = kind("bunker", &["STRUCTURE"]);
    "Heavy".clone_into(&mut bunker.armor_type);
    bunker.health = q(400);
    bunker.max_health = q(400);
    let soft = kind("soft", &["STRUCTURE"]);
    let mut w = World::empty();
    w.units = [howitzer, launcher, bunker, soft]
        .into_iter()
        .map(|k| (k.id.clone(), k))
        .collect();
    w.projectiles = [shell, missile]
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();
    w.map = flat_map(64);
    w.armour = BTreeMap::from([(
        "Heavy".to_owned(),
        BTreeMap::from([("Normal".to_owned(), Q::from_num(0.5))]),
    )]);
    Arc::new(w)
}

/// Run the scenario and return the final digest and the state.
fn run() -> (Digest, State) {
    let mut s = State::with_world(world(), 1);
    for army in [1, 2] {
        s.add_army(Army(army), q(0), q(0), q(0), q(0));
    }
    let howitzer = s.spawn(Army(1), "howitzer", pos(10.5, 0.0, 10.5), q(100));
    let launcher = s.spawn(Army(1), "launcher", pos(10.5, 0.0, 30.5), q(100));
    // The bunker is armoured and has a neighbour inside the shell's radius;
    // the missiles kill the weak target, then the neighbour.
    let bunker = s.spawn(Army(2), "bunker", pos(40.5, 0.0, 10.5), q(400));
    s.spawn(Army(2), "soft", pos(42.5, 0.0, 11.5), q(100));
    let weak = s.spawn(Army(2), "soft", pos(35.5, 0.0, 35.5), q(80));
    let mut digest = Digest::default();
    for tick in 0..TICKS {
        let cmds = if tick == 0 {
            vec![
                order_cmd(
                    0,
                    0,
                    howitzer,
                    Order::Attack { target: bunker },
                    QueueMode::Replace,
                ),
                order_cmd(
                    0,
                    1,
                    launcher,
                    Order::Attack { target: weak },
                    QueueMode::Replace,
                ),
            ]
        } else {
            Vec::new()
        };
        digest = step(&mut s, &cmds);
    }
    (digest, s)
}

/// The scenario's final digest equals the pinned literal, twice over, and
/// the scenario does what its description says: the missiles kill both
/// soft targets, and the shells and the missiles wear the armoured bunker
/// down.
#[test]
fn sim_combat_golden_digest_is_pinned() {
    let (first, s) = run();
    let (second, _) = run();
    assert_eq!(first, second, "two runs agree");
    assert!(
        s.counters.projectile_impacts >= 5,
        "shells and missiles hit"
    );
    let bunker = s
        .entities
        .values()
        .find(|e| e.blueprint == "bunker")
        .expect("the bunker stands");
    let worn = q(400) - bunker.health;
    assert!(worn > Q::ZERO, "the bunker is worn down");
    assert!(
        s.entities.values().all(|e| e.blueprint != "soft"),
        "the missiles killed the weak target, then took the next"
    );
    assert_eq!(s.counters.deaths, 2);
    assert_eq!(first, GOLDEN, "the pinned digest: {first:?}");
}
