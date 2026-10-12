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

use supercom_bp::{Key, Value, parse_bytes};
use supercom_data::{Kind, Projectile, Unit, projectile_from_value, unit_from_value, validate};
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

/// The 568 installed units, projected; the assertion names the first
/// refusal, so a short result is itself a failure.
fn projected_units(fa: &Path) -> Vec<Unit> {
    let (_, bodies) = unit_bodies(fa);
    let units: Vec<Unit> = bodies
        .iter()
        .filter_map(|(id, body)| unit_from_value(id, body).ok())
        .collect();
    assert_eq!(units.len(), 568, "units projecting");
    units
}

/// Every installed projectile, projected, from the archives the importer
/// reads them from: 287 in `projectiles.scd` and 78 in `effects.scd`.
fn projected_projectiles(fa: &Path) -> Vec<Projectile> {
    let mut out = Vec::new();
    for (name, expected) in [("projectiles.scd", 287), ("effects.scd", 78)] {
        let path = fa.join("gamedata").join(name);
        let archive = Archive::open(&path);
        assert!(
            archive.is_ok(),
            "Archive::open({}) must succeed, got {:?}",
            path.display(),
            archive.as_ref().err()
        );
        let archive = archive.expect("opened: asserted above");
        let members: Vec<String> = archive
            .entries()
            .filter(|e| e.name.to_ascii_lowercase().ends_with("_proj.bp"))
            .map(|e| e.name.clone())
            .collect();
        assert_eq!(members.len(), expected, "{name} projectile members");
        for member in &members {
            let bytes = archive.read(member);
            assert!(
                bytes.is_ok(),
                "reading {member}: {:?}",
                bytes.as_ref().err()
            );
            let bp = parse_bytes(&bytes.expect("read: asserted above"));
            assert!(bp.is_ok(), "parsing {member}: {:?}", bp.as_ref().err());
            let body = bp.expect("parsed: asserted above").body;
            let projected = projectile_from_value(member, &body);
            assert!(
                projected.is_ok(),
                "projecting {member}: {:?}",
                projected.as_ref().err()
            );
            out.push(projected.expect("projected: asserted above"));
        }
    }
    assert_eq!(out.len(), 365, "projectiles projecting");
    out
}

/// How many of `items` yield a value for `field`.
fn count_some<T, V>(items: &[T], field: impl Fn(&T) -> Option<V>) -> usize {
    items.iter().filter(|i| field(i).is_some()).count()
}

/// The smallest and largest of the values `field` yields.
fn range_of<T>(items: &[T], field: impl Fn(&T) -> Option<f64>) -> Option<(f64, f64)> {
    items
        .iter()
        .filter_map(field)
        .fold(None, |acc, v| match acc {
            None => Some((v, v)),
            Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
        })
}

/// Pins the build and placement fields (DAT-032): `BuildableCategory` in
/// 105 of 568 units (215 expressions), `MaintenanceConsumptionPerSecondEnergy`
/// in 84, and `Physics.BuildRestriction` in 16 (12 on a mass deposit, 4 on a
/// hydrocarbon deposit), each counted through the projection.
#[test]
fn conformance_data_build_and_placement_fields() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let units = projected_units(&fa);
    let with_list = units
        .iter()
        .filter(|u| !u.economy.buildable_category.is_empty());
    assert_eq!(with_list.count(), 105, "units with BuildableCategory");
    let entries: usize = units
        .iter()
        .map(|u| u.economy.buildable_category.len())
        .sum();
    assert_eq!(entries, 215, "BuildableCategory expressions");
    assert_eq!(
        count_some(&units, |u| u
            .economy
            .maintenance_consumption_per_second_energy),
        84,
        "units with MaintenanceConsumptionPerSecondEnergy"
    );
    assert_eq!(
        range_of(&units, |u| u
            .economy
            .maintenance_consumption_per_second_energy),
        Some((0.0, 5000.0)),
        "MaintenanceConsumptionPerSecondEnergy range"
    );
    let restriction = |wanted: &str| {
        units
            .iter()
            .filter(|u| u.physics.build_restriction.as_deref() == Some(wanted))
            .count()
    };
    assert_eq!(restriction("RULEUBR_OnMassDeposit"), 12, "OnMassDeposit");
    assert_eq!(
        restriction("RULEUBR_OnHydrocarbonDeposit"),
        4,
        "OnHydrocarbonDeposit"
    );
    assert_eq!(
        count_some(&units, |u| u.physics.build_restriction.clone()),
        16,
        "units with BuildRestriction"
    );
}

/// Pins the physics and footprint fields (DAT-033): `MaxAcceleration` 149,
/// `MaxBrake` 124, `Elevation` 95, `MinSpeedPercent` 519; a `Footprint` in
/// 402 units with `SizeX` and `SizeZ` in 363 and `MinWaterDepth` in 48.
#[test]
fn conformance_data_physics_and_footprint_fields() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let units = projected_units(&fa);
    assert_eq!(
        count_some(&units, |u| u.physics.max_acceleration),
        149,
        "MaxAcceleration"
    );
    assert_eq!(
        range_of(&units, |u| u.physics.max_acceleration),
        Some((0.38, 10.5)),
        "MaxAcceleration range"
    );
    assert_eq!(count_some(&units, |u| u.physics.max_brake), 124, "MaxBrake");
    assert_eq!(
        range_of(&units, |u| u.physics.max_brake),
        Some((1.0, 10.0)),
        "MaxBrake range"
    );
    assert_eq!(count_some(&units, |u| u.physics.elevation), 95, "Elevation");
    assert_eq!(
        range_of(&units, |u| u.physics.elevation),
        Some((-5.0, 75.0)),
        "Elevation range"
    );
    assert_eq!(
        count_some(&units, |u| u.physics.min_speed_percent),
        519,
        "MinSpeedPercent"
    );
    let footprints: Vec<_> = units.iter().filter_map(|u| u.footprint.as_ref()).collect();
    assert_eq!(footprints.len(), 402, "units with a Footprint");
    assert_eq!(count_some(&footprints, |f| f.size_x), 363, "SizeX");
    assert_eq!(count_some(&footprints, |f| f.size_z), 363, "SizeZ");
    assert_eq!(
        count_some(&footprints, |f| f.min_water_depth),
        48,
        "MinWaterDepth"
    );
    assert_eq!(
        range_of(&footprints, |f| f.min_water_depth),
        Some((0.0, 2.0)),
        "MinWaterDepth range"
    );
}

/// Pins the command capabilities (DAT-034): all 568 units carry a
/// `General.CommandCaps` table, 22 distinct keys over all of them, of which
/// 21 are true on at least one unit (`RULEUCC_Teleport` is present on 4
/// units and true on none); the projection lists the true keys, sorted.
#[test]
fn conformance_data_command_caps_shape() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (_, bodies) = unit_bodies(&fa);
    let caps_of = |body: &Value| match body.get("General").and_then(|g| g.get("CommandCaps")) {
        Some(Value::Table(entries)) => Some(entries.clone()),
        _ => None,
    };
    let tables: Vec<_> = bodies.iter().filter_map(|(_, b)| caps_of(b)).collect();
    assert_eq!(tables.len(), 568, "units with CommandCaps");
    let present: BTreeSet<String> = tables
        .iter()
        .flatten()
        .filter_map(|(k, _)| match k {
            Key::Name(n) => Some(n.clone()),
            Key::Index(_) => None,
        })
        .collect();
    assert_eq!(present.len(), 22, "distinct CommandCaps keys present");
    let teleport = tables
        .iter()
        .filter(|t| {
            t.iter()
                .any(|(k, _)| *k == Key::Name("RULEUCC_Teleport".into()))
        })
        .count();
    assert_eq!(teleport, 4, "units naming RULEUCC_Teleport");

    let units = projected_units(&fa);
    let mut granted: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for unit in &units {
        let mut sorted = unit.command_caps.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(unit.command_caps, sorted, "{} command_caps sorted", unit.id);
        for cap in &unit.command_caps {
            *granted.entry(cap.as_str()).or_default() += 1;
        }
    }
    let expected: std::collections::BTreeMap<&str, usize> = [
        ("RULEUCC_Move", 235),
        ("RULEUCC_Attack", 191),
        ("RULEUCC_Guard", 227),
        ("RULEUCC_Patrol", 228),
        ("RULEUCC_Stop", 294),
        ("RULEUCC_RetaliateToggle", 236),
        ("RULEUCC_CallTransport", 137),
        ("RULEUCC_Pause", 113),
        ("RULEUCC_Repair", 30),
        ("RULEUCC_Reclaim", 29),
        ("RULEUCC_Capture", 27),
        ("RULEUCC_Transport", 21),
        ("RULEUCC_Dive", 13),
        ("RULEUCC_Dock", 42),
        ("RULEUCC_Ferry", 10),
        ("RULEUCC_Nuke", 9),
        ("RULEUCC_SiloBuildNuke", 9),
        ("RULEUCC_SiloBuildTactical", 8),
        ("RULEUCC_Overcharge", 4),
        ("RULEUCC_Tactical", 4),
        ("RULEUCC_Sacrifice", 3),
    ]
    .into_iter()
    .collect();
    assert_eq!(granted, expected, "units granted each capability");
}

/// Pins the layer capabilities (DAT-039): all 568 installed units carry a
/// `Physics.BuildOnLayerCaps` table of booleans over the six keys of
/// SIM-030, and the projection lists the true keys of each unit, sorted.
#[test]
fn conformance_data_build_on_layer_caps() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (_, bodies) = unit_bodies(&fa);
    let tables: Vec<_> = bodies
        .iter()
        .filter_map(
            |(_, b)| match b.get("Physics").and_then(|p| p.get("BuildOnLayerCaps")) {
                Some(Value::Table(entries)) => Some(entries.clone()),
                _ => None,
            },
        )
        .collect();
    assert_eq!(tables.len(), 568, "units with BuildOnLayerCaps");
    let mut true_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for (key, value) in tables.iter().flatten() {
        let Key::Name(name) = key else {
            panic!("a positional BuildOnLayerCaps key");
        };
        let Value::Bool(b) = value else {
            panic!("{name} is not a boolean: {value:?}");
        };
        *true_counts.entry(name.clone()).or_default() += usize::from(*b);
    }
    let expected: std::collections::BTreeMap<String, usize> = [
        ("LAYER_Air", 62),
        ("LAYER_Land", 440),
        ("LAYER_Orbit", 0),
        ("LAYER_Seabed", 23),
        ("LAYER_Sub", 18),
        ("LAYER_Water", 87),
    ]
    .into_iter()
    .map(|(k, n)| (k.to_owned(), n))
    .collect();
    assert_eq!(true_counts, expected, "true count per BuildOnLayerCaps key");

    let mut projected: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for unit in projected_units(&fa) {
        let mut sorted = unit.physics.build_on_layer_caps.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            unit.physics.build_on_layer_caps, sorted,
            "{} sorted",
            unit.id
        );
        for cap in &unit.physics.build_on_layer_caps {
            *projected.entry(cap.clone()).or_default() += 1;
        }
    }
    projected.entry("LAYER_Orbit".to_owned()).or_default();
    assert_eq!(projected, expected, "units projecting each layer");
}

type WeaponNumber = fn(&supercom_data::Weapon) -> Option<f64>;
type WeaponFlag = fn(&supercom_data::Weapon) -> Option<bool>;
type ProjectileNumber = fn(&Projectile) -> Option<f64>;
type ProjectileFlag = fn(&Projectile) -> Option<bool>;

/// Every installed weapon entry as projected, in unit order.
fn projected_weapons(fa: &Path) -> Vec<supercom_data::Weapon> {
    let weapons: Vec<_> = projected_units(fa)
        .into_iter()
        .flat_map(|u| u.weapons)
        .collect();
    assert_eq!(weapons.len(), 494, "weapon entries");
    weapons
}

/// The weapon scalar fields (DAT-035) over the 494 installed weapon
/// entries: each field's presence count and, where the data shows one, its
/// range.
fn assert_weapon_scalars(w: &[supercom_data::Weapon]) {
    let numbers: [(&str, usize, WeaponNumber); 11] = [
        ("DamageRadius", 266, |w| w.damage_radius),
        ("MinRadius", 87, |w| w.min_radius),
        ("MuzzleVelocity", 383, |w| w.muzzle_velocity),
        ("ProjectilesPerOnFire", 255, |w| w.projectiles_per_on_fire),
        ("MuzzleSalvoSize", 402, |w| w.muzzle_salvo_size),
        ("MuzzleSalvoDelay", 402, |w| w.muzzle_salvo_delay),
        ("RackSalvoReloadTime", 393, |w| w.rack_salvo_reload_time),
        ("RackSalvoChargeTime", 393, |w| w.rack_salvo_charge_time),
        ("TrackingRadius", 309, |w| w.tracking_radius),
        ("FiringTolerance", 457, |w| w.firing_tolerance),
        ("TargetCheckInterval", 404, |w| w.target_check_interval),
    ];
    for (field, expected, get) in numbers {
        assert_eq!(count_some(w, get), expected, "{field} present");
    }
    assert_eq!(
        range_of(w, |w| w.damage_radius),
        Some((0.0, 20.0)),
        "DamageRadius range"
    );
    assert_eq!(
        range_of(w, |w| w.muzzle_velocity),
        Some((0.0, 160.0)),
        "MuzzleVelocity range"
    );
    assert_eq!(
        range_of(w, |w| w.projectiles_per_on_fire),
        Some((1.0, 25.0)),
        "ProjectilesPerOnFire range"
    );
    assert_eq!(
        range_of(w, |w| w.muzzle_salvo_size),
        Some((0.0, 15.0)),
        "MuzzleSalvoSize range"
    );
    let flags: [(&str, usize, WeaponFlag); 4] = [
        ("DamageFriendly", 188, |w| w.damage_friendly),
        ("RackSalvoFiresAfterCharge", 24, |w| {
            w.rack_salvo_fires_after_charge
        }),
        ("Turreted", 399, |w| w.turreted),
        ("ManualFire", 22, |w| w.manual_fire),
    ];
    for (field, expected, get) in flags {
        assert_eq!(count_some(w, get), expected, "{field} present");
    }
}

/// The weapon string and list fields (DAT-035): the damage types, the
/// ballistic arcs, the 12 weapon categories, the target priorities (337
/// weapons, 2,036 expressions, 21 distinct), the layer caps table (395
/// weapons, 747 entries over the five layers), and `ManualFire`'s two stored
/// kinds, 13 booleans and 9 numbers (all 1), all reading as true.
fn assert_weapon_lists(fa: &Path, w: &[supercom_data::Weapon]) {
    let tally = |get: &dyn Fn(&supercom_data::Weapon) -> Option<String>| {
        let mut counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::default();
        for weapon in w {
            if let Some(v) = get(weapon) {
                *counts.entry(v).or_default() += 1;
            }
        }
        counts
    };
    let types = tally(&|w| (!w.damage_type.is_empty()).then(|| w.damage_type.clone()));
    let expected_types: std::collections::BTreeMap<String, usize> = [
        ("Normal", 484),
        ("Overcharge", 5),
        ("Deathnuke", 4),
        ("EMP", 1),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    assert_eq!(types, expected_types, "DamageType values");
    let arcs = tally(&|w| w.ballistic_arc.clone());
    let expected_arcs: std::collections::BTreeMap<String, usize> = [
        ("RULEUBA_None", 294),
        ("RULEUBA_LowArc", 83),
        ("RULEUBA_HighArc", 16),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    assert_eq!(arcs, expected_arcs, "BallisticArc values");
    let categories = tally(&|w| (!w.weapon_category.is_empty()).then(|| w.weapon_category.clone()));
    assert_eq!(categories.len(), 12, "WeaponCategory distinct values");
    assert_eq!(categories.values().sum::<usize>(), 494, "WeaponCategory");

    let with_priorities = w.iter().filter(|w| !w.target_priorities.is_empty());
    assert_eq!(
        with_priorities.count(),
        337,
        "weapons with TargetPriorities"
    );
    let priorities: Vec<&String> = w.iter().flat_map(|w| &w.target_priorities).collect();
    assert_eq!(priorities.len(), 2036, "TargetPriorities expressions");
    let distinct: BTreeSet<&&String> = priorities.iter().collect();
    assert_eq!(distinct.len(), 21, "distinct TargetPriorities expressions");

    let with_layers = w.iter().filter(|w| !w.fire_target_layer_caps.is_empty());
    assert_eq!(with_layers.count(), 395, "weapons with a layer caps table");
    let entries: usize = w.iter().map(|w| w.fire_target_layer_caps.len()).sum();
    assert_eq!(entries, 747, "layer caps entries");
    let layers: BTreeSet<&str> = w
        .iter()
        .flat_map(|w| w.fire_target_layer_caps.iter().map(|(l, _)| l.as_str()))
        .collect();
    assert_eq!(
        layers,
        ["Air", "Land", "Seabed", "Sub", "Water"]
            .into_iter()
            .collect(),
        "layer names"
    );
    assert!(
        w.iter()
            .all(|w| w.fire_target_layer_caps.windows(2).all(|p| p[0].0 < p[1].0)),
        "layer caps sorted by layer name"
    );

    // Parser level: the two stored kinds of ManualFire.
    let (_, bodies) = unit_bodies(fa);
    let (mut booleans, mut numbers) = (0, 0);
    for (_, body) in &bodies {
        if let Some(Value::Table(entries)) = body.get("Weapon") {
            for (_, weapon) in entries {
                match weapon.get("ManualFire") {
                    Some(Value::Bool(_)) => booleans += 1,
                    Some(Value::Number(_)) => numbers += 1,
                    _ => {}
                }
            }
        }
    }
    assert_eq!((booleans, numbers), (13, 9), "ManualFire stored kinds");
    let manual = w.iter().filter(|w| w.manual_fire == Some(true));
    assert_eq!(manual.count(), 22, "weapons reading ManualFire as true");
}

/// Pins the weapon fields (DAT-035): the scalars and flags, then the strings
/// and lists, over the 494 installed weapon entries.
#[test]
fn conformance_data_weapon_fields() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let weapons = projected_weapons(&fa);
    assert_weapon_scalars(&weapons);
    assert_weapon_lists(&fa, &weapons);
}

/// Pins the projectile physics (DAT-036): all 365 projectiles carry the
/// table and the fields are present in the measured numbers.
#[test]
fn conformance_data_projectile_physics_fields() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let p = projected_projectiles(&fa);
    let numbers: [(&str, usize, ProjectileNumber); 5] = [
        ("InitialSpeed", 287, |p| p.physics.initial_speed),
        ("MaxSpeed", 175, |p| p.physics.max_speed),
        ("Acceleration", 173, |p| p.physics.acceleration),
        ("TurnRate", 249, |p| p.physics.turn_rate),
        ("Lifetime", 150, |p| p.physics.lifetime),
    ];
    for (field, expected, get) in numbers {
        assert_eq!(count_some(&p, get), expected, "{field} present");
    }
    assert_eq!(
        range_of(&p, |p| p.physics.initial_speed),
        Some((0.0, 120.0)),
        "InitialSpeed range"
    );
    assert_eq!(
        range_of(&p, |p| p.physics.max_speed),
        Some((0.0, 600.0)),
        "MaxSpeed range"
    );
    let flags: [(&str, usize, ProjectileFlag); 5] = [
        ("TrackTarget", 153, |p| p.physics.track_target),
        ("UseGravity", 219, |p| p.physics.use_gravity),
        ("LeadTarget", 111, |p| p.physics.lead_target),
        ("VelocityAlign", 327, |p| p.physics.velocity_align),
        ("DestroyOnWater", 364, |p| p.physics.destroy_on_water),
    ];
    for (field, expected, get) in flags {
        assert_eq!(count_some(&p, get), expected, "{field} present");
    }
}
