//! Conformance of `supercom-bp` against the installed game.
//!
//! Every test here is game-dependent: it begins by asking `supercom_testkit`
//! for the install and returns (skips, loudly) when there is none. Run them
//! against the real game with `SUPERCOM_REQUIRE_GAME=1`.
//!
//! The expected numbers are measured on the installed Forged Alliance: 568
//! unit blueprints in `units.scd`; 365 projectile blueprints (287 in
//! `projectiles.scd`, 78 in `effects.scd`); 336 prop blueprints (334 in
//! `env.scd`, 1 in `meshes.scd`, 1 in `props.scd`); 61 `_save.lua` and 61
//! `_scenario.lua` files under `maps/`, whose save files hold 19,363 markers.
//! No game content is stored here: names, counts, and key names only.
//!
//! Each test enumerates the whole set, never a sample, and asserts the exact
//! count of files that parse.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::fs;
use std::path::{Path, PathBuf};

use supercom_bp::{Value, markers, parse_bytes, parse_script};
use supercom_scd::Archive;

/// Opens one archive of the install, failing the calling test on error.
fn opened(fa: &Path, name: &str) -> Archive {
    let path = fa.join("gamedata").join(name);
    let archive = Archive::open(&path);
    assert!(
        archive.is_ok(),
        "Archive::open({}) must succeed, got {:?}",
        path.display(),
        archive.as_ref().err()
    );
    archive.expect("opened: asserted above")
}

/// The contents of every member of `archives` whose name ends in `suffix`
/// (ignoring case), as `(member name, bytes)`.
fn members(fa: &Path, archives: &[&str], suffix: &str) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    for name in archives {
        let archive = opened(fa, name);
        let wanted: Vec<String> = archive
            .entries()
            .filter(|e| e.name.to_ascii_lowercase().ends_with(suffix))
            .map(|e| e.name.clone())
            .collect();
        for member in wanted {
            let bytes = archive.read(&member);
            assert!(
                bytes.is_ok(),
                "reading {member} from {name}: {:?}",
                bytes.as_ref().err()
            );
            found.push((member, bytes.expect("read: asserted above")));
        }
    }
    found
}

/// How many of `files` parse as blueprints whose body is a table under the
/// constructor `expected`.
fn count_parsing_as(files: &[(String, Vec<u8>)], expected: &str) -> usize {
    files
        .iter()
        .filter(|(_, bytes)| {
            parse_bytes(bytes)
                .is_ok_and(|bp| bp.constructor == expected && matches!(bp.body, Value::Table(_)))
        })
        .count()
}

/// The `.lua` files named `*<suffix>` in the folders of `<fa>/maps`, as
/// `(path, text decoded from ISO-8859-1)`.
fn map_scripts(fa: &Path, suffix: &str) -> Vec<(PathBuf, String)> {
    let folders = fs::read_dir(fa.join("maps"));
    assert!(
        folders.is_ok(),
        "cannot list maps: {:?}",
        folders.as_ref().err()
    );
    let mut paths = Vec::new();
    for folder in folders.expect("listed: asserted above").flatten() {
        let Ok(inner) = fs::read_dir(folder.path()) else {
            continue;
        };
        for file in inner.flatten() {
            let name = file.file_name().to_string_lossy().to_ascii_lowercase();
            if name.ends_with(suffix) {
                paths.push(file.path());
            }
        }
    }
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path);
            assert!(
                bytes.is_ok(),
                "reading {}: {:?}",
                path.display(),
                bytes.as_ref().err()
            );
            let text = bytes
                .expect("read: asserted above")
                .iter()
                .map(|&b| char::from(b))
                .collect();
            (path, text)
        })
        .collect()
}

/// Pins the unit set: all 568 `_unit.bp` members of `units.scd` parse, each
/// as a table under the constructor `UnitBlueprint`.
#[test]
fn conformance_bp_every_unit_parses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let files = members(&fa, &["units.scd"], "_unit.bp");
    assert_eq!(files.len(), 568, "units enumerated");
    assert_eq!(count_parsing_as(&files, "UnitBlueprint"), 568);
}

/// Pins the projectile set: all 365 `_proj.bp` members of `projectiles.scd`
/// (287) and `effects.scd` (78) parse as tables under `ProjectileBlueprint`.
#[test]
fn conformance_bp_every_projectile_parses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let files = members(&fa, &["projectiles.scd", "effects.scd"], "_proj.bp");
    assert_eq!(files.len(), 365, "projectiles enumerated");
    assert_eq!(count_parsing_as(&files, "ProjectileBlueprint"), 365);
}

/// Pins the prop set: all 336 `_prop.bp` members of `env.scd` (334),
/// `meshes.scd` (1), and `props.scd` (1) parse as tables under
/// `PropBlueprint`.
#[test]
fn conformance_bp_every_prop_parses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let files = members(&fa, &["env.scd", "meshes.scd", "props.scd"], "_prop.bp");
    assert_eq!(files.len(), 336, "props enumerated");
    assert_eq!(count_parsing_as(&files, "PropBlueprint"), 336);
}

/// Pins the unit key census: `Categories`, `Defense`, `Economy`, `General`,
/// and `Physics` appear in all 568 parsed unit bodies, `Intel` in exactly
/// 395, and `Weapon` in exactly 247.
#[test]
fn conformance_bp_unit_keys_present() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let files = members(&fa, &["units.scd"], "_unit.bp");
    let bodies: Vec<Value> = files
        .iter()
        .filter_map(|(_, bytes)| parse_bytes(bytes).ok())
        .map(|bp| bp.body)
        .collect();
    assert_eq!(bodies.len(), 568, "unit bodies parsed");
    let having = |key: &str| bodies.iter().filter(|b| b.get(key).is_some()).count();
    for key in ["Categories", "Defense", "Economy", "General", "Physics"] {
        assert_eq!(having(key), 568, "units with {key}");
    }
    assert_eq!(having("Intel"), 395, "units with Intel");
    assert_eq!(having("Weapon"), 247, "units with Weapon");
}

/// Pins the map save files: all 61 `*_save.lua` under `maps/` parse with
/// `parse_script`.
#[test]
fn conformance_bp_every_save_parses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let files = map_scripts(&fa, "_save.lua");
    assert_eq!(files.len(), 61, "save files enumerated");
    let parsed = files
        .iter()
        .filter(|(_, text)| parse_script(text).is_ok())
        .count();
    assert_eq!(parsed, 61);
}

/// Pins the map scenario files: all 61 `*_scenario.lua` under `maps/` parse
/// with `parse_script`.
#[test]
fn conformance_bp_every_scenario_parses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let files = map_scripts(&fa, "_scenario.lua");
    assert_eq!(files.len(), 61, "scenario files enumerated");
    let parsed = files
        .iter()
        .filter(|(_, text)| parse_script(text).is_ok())
        .count();
    assert_eq!(parsed, 61);
}

/// Pins the marker census: `markers` over all 61 save files yields exactly
/// 19,363 markers (the composer's regex count and an independent throwaway
/// parser agree), each with a non-empty name and kind and a finite position.
#[test]
fn conformance_bp_markers_extracted() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let files = map_scripts(&fa, "_save.lua");
    assert_eq!(files.len(), 61, "save files enumerated");
    let mut total = 0;
    let mut malformed = 0;
    for (_, text) in &files {
        let Ok(script) = parse_script(text) else {
            continue;
        };
        let Ok(found) = markers(&script) else {
            continue;
        };
        total += found.len();
        malformed += found
            .iter()
            .filter(|m| {
                m.name.is_empty() || m.kind.is_empty() || !m.position.iter().all(|c| c.is_finite())
            })
            .count();
    }
    assert_eq!(total, 19_363, "markers extracted");
    assert_eq!(
        malformed, 0,
        "markers with an empty name or kind or a non-finite position"
    );
}
