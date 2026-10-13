//! Guards on progress and the snapshot (docs/specs/sim.md, SIM-046,
//! SIM-056, SIM-059). All synthetic.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use supercom_sim::{Army, BUILD_TIME_FLOOR, EntityId, Q, QueueMode, total_work};
use support::{
    build_at, builder, flat_map, frac, kind, order_cmd, pos, q, state, step, structure, world,
};

/// Completion and progress share one total: the build time, floored.
#[test]
fn total_work_is_the_floored_build_time() {
    let mut k = kind("k", &["STRUCTURE"]);
    k.build_time = Some(q(60));
    assert_eq!(total_work(&k), q(60));
    // 0.05 is below the floor: raw (1 << 32) / 20 = 214748364.
    k.build_time = Some(Q::from_bits(214_748_364));
    assert_eq!(total_work(&k), BUILD_TIME_FLOOR);
    k.build_time = None;
    assert_eq!(total_work(&k), BUILD_TIME_FLOOR);
}

/// Progress of a kind whose build time is below the floor shows against
/// the floor, and divides without overflow.
#[test]
fn progress_divides_against_the_floor_and_saturates() {
    let mut quick = structure("quick", 1, 1, 1);
    quick.build_time = Some(Q::from_bits(214_748_364));
    let w = world(vec![quick.clone()], Vec::new(), flat_map(8));
    let mut s = state(&w, Q::ZERO, Q::ZERO);
    let id = s
        .spawn_under_construction(Army(1), "quick", pos(1.5, 0.0, 1.5))
        .expect("a build time is set");
    let site = s.entities.get_mut(&id).expect("spawned");
    // Half the floor, 429496730 / 2 = 214748365 raw, is half the progress:
    // 214748365 << 32 over 429496730 is exactly 1 << 31.
    site.built = Q::from_bits(214_748_365);
    assert_eq!(site.progress(&quick), frac(1, 2));
    // The largest value over a tenth overflows a plain division.
    site.built = Q::MAX;
    assert_eq!(site.progress(&quick), Q::ONE);
}

/// A snapshot taken mid-build carries the work done, completion, both
/// storages, and the ledger.
#[test]
fn a_mid_build_snapshot_carries_work_storage_and_ledger() {
    let w = world(
        vec![
            builder("worker", 10, &["gen"]),
            structure("gen", 60, 600, 60),
        ],
        Vec::new(),
        flat_map(32),
    );
    let mut s = state(&w, q(1_000_000), q(1_000_000));
    let b = s.spawn(Army(1), "worker", pos(4.5, 0.0, 4.5), q(100));
    step(
        &mut s,
        &[order_cmd(
            0,
            0,
            b,
            build_at("gen", 4.5, 8.5),
            QueueMode::Replace,
        )],
    );
    for _ in 0..3 {
        step(&mut s, &[]);
    }
    let snap = s.snapshot();
    let site = snap
        .entities
        .iter()
        .find(|e| e.id == EntityId(2))
        .expect("the site is in the snapshot");
    // Three work ticks of per_tick(10) = 1.
    assert_eq!(site.built, q(3));
    assert!(!site.complete);
    let army = snap.armies.first().expect("army 1 is in the snapshot");
    // The worker kind's storage, from the helper: 100000 and 1000000.
    assert_eq!(army.mass_storage, q(100_000));
    assert_eq!(army.energy_storage, q(1_000_000));
    // The site costs 60 mass and 600 energy over 6 seconds: 1 and 10 per
    // tick at rate 10, and the builder is fully supplied.
    assert_eq!(army.ledger.requested_mass, q(1));
    assert_eq!(army.ledger.requested_energy, q(10));
    assert_eq!(army.ledger.granted_mass, q(1));
    assert_eq!(army.ledger.fraction_mass, Q::ONE);
}
