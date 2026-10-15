//! A sweep over falling shells (docs/specs/sim.md, SIM-073, SIM-076,
//! SIM-077): for a spread of shell speeds, every target on flat ground at
//! a planar range up to the speed squared over 4.9 is hit by the shell a
//! turret fires through the weapons, projectiles, and damage phases. The
//! ranges include ones that are not multiples of the shell's step (its
//! speed times 0.1), where a flight model that sinks below the parabola
//! misses. All synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::sync::Arc;

use supercom_sim::{
    Army, LayerSet, Order, ProjectileKind, QueueMode, State, UnitKind, WeaponKind, World,
};
use support::{flat_map, kind, order_cmd, pos, q, step};

const DAMAGE: i64 = 10;
const FIRER_X: f64 = 5.5;
const Z: f64 = 10.5;
/// Gravity, positive, in world units per second squared (SIM-073).
const G: f64 = 4.9;

/// The speeds of the sweep: slow shells with a short reach to fast ones
/// whose reach nearly spans the map.
const SPEEDS: [f64; 10] = [4.0, 5.0, 6.5, 7.5, 9.0, 10.0, 11.3, 12.5, 14.0, 15.0];

/// The ranges of one speed: a point-blank one, then for each multiple of
/// the step a range on the step and three off it, then the reach itself.
fn ranges(speed: f64) -> Vec<f64> {
    let step_len = speed * 0.1;
    let reach = speed * speed / G;
    let mut out = vec![0.5];
    let mut k = 1.0;
    while k * step_len <= reach {
        for off in [0.0, 0.13, 0.5, 0.87] {
            let r = k * step_len + off;
            if r <= reach {
                out.push(r);
            }
        }
        k += 1.0;
    }
    out.push(reach);
    out
}

fn world_for(speed: f64) -> Arc<World> {
    let mut shell = ProjectileKind {
        id: "shell".to_owned(),
        initial_speed: Some(speed),
        use_gravity: true,
        ..ProjectileKind::default()
    };
    shell.lifetime = None;
    let cannon = WeaponKind {
        category: "Artillery".to_owned(),
        damage: q(DAMAGE),
        damage_type: "Normal".to_owned(),
        rate_of_fire: Some(0.03),
        max_radius: Some(100.0),
        muzzle_velocity: Some(speed),
        projectile_id: Some("shell".to_owned()),
        target_layers: LayerSet::from_caps("Land").expect("caps"),
        ..WeaponKind::default()
    };
    let mut turret: UnitKind = kind("turret", &["STRUCTURE"]);
    turret.weapons = vec![cannon];
    let mut w = World::empty();
    w.units = [turret, kind("dummy", &["STRUCTURE"])]
        .into_iter()
        .map(|k| (k.id.clone(), k))
        .collect();
    w.projectiles = [("shell".to_owned(), shell)].into_iter().collect();
    w.map = flat_map(64);
    Arc::new(w)
}

/// Fire one shell at a target `range` away and report whether it landed
/// on the target (the only impact that hurts it, the shell having no
/// damage radius).
fn hits(world: &Arc<World>, range: f64) -> bool {
    let mut s = State::with_world(Arc::clone(world), 1);
    for army in [1, 2] {
        s.add_army(Army(army), q(0), q(0), q(0), q(0));
    }
    let turret = s.spawn(Army(1), "turret", pos(FIRER_X, 0.0, Z), q(100));
    let target = s.spawn(Army(2), "dummy", pos(FIRER_X + range, 0.0, Z), q(1000));
    let attack = order_cmd(0, 0, turret, Order::Attack { target }, QueueMode::Replace);
    step(&mut s, &[attack]);
    for _ in 0..200 {
        if s.counters.projectile_impacts > 0 || s.projectiles.is_empty() {
            break;
        }
        step(&mut s, &[]);
    }
    s.counters.projectile_impacts == 1
        && s.entities.get(&target).expect("the target").health == q(1000 - DAMAGE)
}

/// SIM-077: every pair of speed and range within reach hits its target.
#[test]
fn sim_falling_shell_hits_every_range_within_reach() {
    let mut pairs = 0_usize;
    let mut missed = Vec::new();
    for speed in SPEEDS {
        let world = world_for(speed);
        for range in ranges(speed) {
            pairs += 1;
            if !hits(&world, range) {
                missed.push((speed, range));
            }
        }
    }
    assert!(
        missed.is_empty(),
        "missed {} of {pairs}: {missed:?}",
        missed.len()
    );
    assert_eq!(pairs, 756, "the sweep's size");
}
