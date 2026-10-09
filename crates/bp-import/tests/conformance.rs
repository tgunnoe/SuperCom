//! Conformance of `supercom-bp-import` against the installed game.
//!
//! Every test here is game-dependent: it begins by asking `supercom_testkit`
//! for the install and returns (skips, loudly) when there is none. Run them
//! against the real game with `SUPERCOM_REQUIRE_GAME=1`.
//!
//! The expected numbers are measured on the installed Forged Alliance: 568
//! units in `units.scd`, 365 projectiles across `projectiles.scd` and
//! `effects.scd`, 336 props across `env.scd`, `meshes.scd`, and `props.scd`.
//! No game content is stored here.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::path::Path;

use supercom_bp_import::{Catalog, cache_key, import, load};
use supercom_scd::Archive;

/// The number of members of `archives` whose name ends in `suffix`, counted
/// through `Archive::open` without the code under test.
fn count_members(fa: &Path, archives: &[&str], suffix: &str) -> usize {
    archives
        .iter()
        .map(|name| {
            let path = fa.join("gamedata").join(name);
            let archive = Archive::open(&path);
            assert!(
                archive.is_ok(),
                "Archive::open({}) must succeed, got {:?}",
                path.display(),
                archive.as_ref().err()
            );
            archive
                .expect("opened: asserted above")
                .entries()
                .filter(|e| e.name.to_ascii_lowercase().ends_with(suffix))
                .count()
        })
        .sum()
}

/// Pins the catalog size: `import` yields 568 units (from `units.scd`
/// alone, so effects.scd's five `_unit.bp` emitters are not units), 365
/// projectiles, and 336 props, matching an independent count of the
/// archives' members.
#[test]
fn conformance_import_catalog_counts() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    assert_eq!(count_members(&fa, &["units.scd"], "_unit.bp"), 568);
    assert_eq!(
        count_members(&fa, &["projectiles.scd", "effects.scd"], "_proj.bp"),
        365
    );
    assert_eq!(
        count_members(&fa, &["env.scd", "meshes.scd", "props.scd"], "_prop.bp"),
        336
    );
    let catalog = import(&fa);
    assert!(
        catalog.is_ok(),
        "import must succeed, got {:?}",
        catalog.as_ref().err()
    );
    let catalog = catalog.expect("imported: asserted above");
    assert_eq!(catalog.units.len(), 568, "units");
    assert_eq!(catalog.projectiles.len(), 365, "projectiles");
    assert_eq!(catalog.props.len(), 336, "props");
}

/// Pins the cache: loading twice into a fresh temporary directory leaves a
/// cache behind after the first call and the second call yields a catalog
/// equal to the first.
#[test]
fn conformance_import_cache_round_trip() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let dir = tempfile::tempdir().expect("temporary directory");
    let first = load(&fa, dir.path());
    assert!(
        first.is_ok(),
        "the first load must succeed, got {:?}",
        first.as_ref().err()
    );
    let first: Catalog = first.expect("loaded: asserted above");
    let cached_files = std::fs::read_dir(dir.path()).map(Iterator::count);
    assert!(
        cached_files.is_ok_and(|n| n > 0),
        "the first load must write a cache"
    );
    let second = load(&fa, dir.path());
    assert!(
        second.is_ok(),
        "the second load must succeed, got {:?}",
        second.as_ref().err()
    );
    assert_eq!(second.expect("loaded: asserted above"), first);
}

/// Pins the cache key: it is non-empty and the same on two calls over an
/// unchanged install.
#[test]
fn conformance_import_cache_key_is_stable() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let first = cache_key(&fa);
    let second = cache_key(&fa);
    assert!(
        first.as_ref().is_ok_and(|k| !k.is_empty()),
        "the key must be non-empty, got {first:?}"
    );
    assert_eq!(first.ok(), second.ok());
}
