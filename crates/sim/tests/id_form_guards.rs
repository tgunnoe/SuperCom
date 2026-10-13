//! An entity's blueprint is stored in one form, ASCII lower case, whatever
//! spelling the caller spawned it with (docs/specs/sim.md, SIM-061).
//! All synthetic.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use supercom_sim::{Army, EntityId};
use support::{builder, flat_map, pos, q, state, step, structure, world};

/// Both spawn paths store the lower-case form, and the economy reads the
/// kind of an entity spawned under an upper-case spelling.
#[test]
fn spawn_stores_the_lower_case_blueprint() {
    let blueprints = world(
        vec![
            builder("worker", 10, &["gen"]),
            structure("gen", 60, 600, 60),
        ],
        Vec::new(),
        flat_map(16),
    );
    let mut s = state(&blueprints, q(0), q(0));
    let worker = s.spawn(Army(1), "WoRkEr", pos(4.5, 0.0, 4.5), q(100));
    let site = s
        .spawn_under_construction(Army(1), "GEN", pos(8.5, 0.0, 8.5))
        .expect("an upper-case spelling finds the kind");
    assert_eq!(worker, EntityId(1));
    assert_eq!(
        s.entities.get(&worker).expect("spawned").blueprint,
        "worker"
    );
    assert_eq!(s.entities.get(&site).expect("spawned").blueprint, "gen");

    step(&mut s, &[]);
    let army = s.armies.get(&Army(1)).expect("army 1 exists");
    // The worker kind's storage, stated by the helper: 100000 and 1000000.
    assert_eq!(army.mass_storage, q(100_000));
    assert_eq!(army.energy_storage, q(1_000_000));
}
