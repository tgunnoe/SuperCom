//! Guard tests of the world loaded from the installed game: every id the
//! world holds is one lower-case form, so every reference resolves
//! (docs/specs/sim.md SIM-011, SIM-049). Each test skips, loudly, without the
//! install and fails if `SUPERCOM_REQUIRE_GAME=1`. Counts and identifiers
//! only; no game content is kept in the tree.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::BTreeSet;

use supercom_sim::World;
use supercom_sim_ingest::load_world;

fn installed_world() -> Option<World> {
    let fa = supercom_testkit::require_game()?;
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load_world(&fa, cache.path(), "SCMP_012").expect("the installed world loads");
    Some((*world).clone())
}

#[test]
fn guard_installed_buildable_members_are_unit_keys() {
    let Some(world) = installed_world() else {
        return;
    };
    for key in world.units.keys().chain(world.projectiles.keys()) {
        assert_eq!(*key, key.to_ascii_lowercase(), "{key} is not lower case");
    }
    let mut selected = BTreeSet::new();
    for kind in world.units.values() {
        for member in &kind.buildable {
            assert!(
                world.units.contains_key(member),
                "{} lists {member}, which is not a unit key",
                kind.id
            );
            selected.insert(member.clone());
        }
    }
    assert_eq!(selected.len(), SELECTED_UNITS, "units some builder selects");
    assert!(
        world
            .units
            .values()
            .filter(|u| selected.contains(&u.id))
            .all(|u| !u.categories.contains("COMMAND")),
        "a COMMAND unit is buildable"
    );
}

#[test]
fn guard_installed_weapon_projectile_ids_are_projectile_keys() {
    let Some(world) = installed_world() else {
        return;
    };
    let mut references = 0;
    for kind in world.units.values() {
        for weapon in &kind.weapons {
            let Some(id) = &weapon.projectile_id else {
                continue;
            };
            references += 1;
            assert!(
                world.projectiles.contains_key(id),
                "{} fires {id}, which is no projectile key",
                kind.id
            );
        }
    }
    assert_eq!(
        references, PROJECTILE_REFERENCES,
        "weapon projectile references"
    );
}

/// Measured at the first run against the install: the units selected by at
/// least one builder, and the weapons naming a projectile.
const SELECTED_UNITS: usize = 370;
const PROJECTILE_REFERENCES: usize = 382;
