//! Conformance of `supercom-data` against the installed game.
//!
//! Every test here is game-dependent: it begins by asking `supercom_testkit`
//! for the install and returns (skips, loudly) when there is none. Run them
//! against the real game with `SUPERCOM_REQUIRE_GAME=1`.
//!
//! The expected numbers are measured on the installed Forged Alliance: 568
//! unit blueprints in `units.scd` whose `Categories` tables hold 152 distinct
//! strings (151 upper-case identifiers and the mixed-case `Cybran`). No game
//! content is stored here.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::BTreeSet;
use std::path::Path;

use supercom_bp::{Value, parse_bytes};
use supercom_data::{Kind, Unit, unit_from_value, validate};
use supercom_scd::Archive;

/// The committed category vocabulary, `nickel/categories.ncl`.
const VOCABULARY: &str = include_str!("../nickel/categories.ncl");

/// Every `_unit.bp` of `units.scd` as `(blueprint id, parsed body)`, the id
/// being the file stem without `_unit`. Bodies that do not parse are left
/// out, so a short result is itself an assertion failure.
fn unit_bodies(fa: &Path) -> (usize, Vec<(String, Value)>) {
    let path = fa.join("gamedata").join("units.scd");
    let archive = Archive::open(&path);
    assert!(
        archive.is_ok(),
        "Archive::open({}) must succeed, got {:?}",
        path.display(),
        archive.as_ref().err()
    );
    let archive = archive.expect("opened: asserted above");
    let names: Vec<String> = archive
        .entries()
        .filter(|e| e.name.to_ascii_lowercase().ends_with("_unit.bp"))
        .map(|e| e.name.clone())
        .collect();
    let mut bodies = Vec::new();
    for name in &names {
        let bytes = archive.read(name);
        assert!(bytes.is_ok(), "reading {name}: {:?}", bytes.as_ref().err());
        let bytes = bytes.expect("read: asserted above");
        if let Ok(bp) = parse_bytes(&bytes) {
            let stem = name.rsplit('/').next().unwrap_or(name);
            let id = stem
                .strip_suffix("_unit.bp")
                .or_else(|| stem.strip_suffix("_Unit.bp"))
                .unwrap_or(stem);
            bodies.push((id.to_owned(), bp.body));
        }
    }
    (names.len(), bodies)
}

/// The tags of `categories.ncl`: the identifier after each `'`.
fn vocabulary_tags() -> BTreeSet<String> {
    let mut tags = BTreeSet::new();
    for line in VOCABULARY
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
    {
        if let Some(rest) = line.trim().strip_prefix('\'') {
            tags.insert(rest.trim_end_matches(',').trim().to_owned());
        }
    }
    tags
}

/// Pins the typed model against the whole unit set: all 568 installed unit
/// bodies pass `validate(Kind::Unit)` and project through `unit_from_value`.
#[test]
fn conformance_data_every_unit_validates() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (enumerated, bodies) = unit_bodies(&fa);
    assert_eq!(enumerated, 568, "units enumerated");
    assert_eq!(bodies.len(), 568, "units parsed");
    let validated = bodies
        .iter()
        .filter(|(_, body)| validate(Kind::Unit, body).is_ok())
        .count();
    assert_eq!(validated, 568, "units passing the contract");
    let projected = bodies
        .iter()
        .filter(|(id, body)| unit_from_value(id, body).is_ok())
        .count();
    assert_eq!(projected, 568, "units projecting");
}

/// Pins the vocabulary: the distinct category strings across the 568
/// installed units equal the committed `categories.ncl` tags, 152 of them.
#[test]
fn conformance_data_category_vocab() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (_, bodies) = unit_bodies(&fa);
    let units: Vec<Unit> = bodies
        .iter()
        .filter_map(|(id, body)| unit_from_value(id, body).ok())
        .collect();
    assert_eq!(units.len(), 568, "units projecting");
    let seen: BTreeSet<String> = units
        .iter()
        .flat_map(|u| u.categories.iter().cloned())
        .collect();
    let committed = vocabulary_tags();
    assert_eq!(committed.len(), 152, "committed tags");
    assert_eq!(seen, committed);
}
