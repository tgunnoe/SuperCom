//! The Rust checks and the Nickel contract agree on every body but the one
//! constraint only the contract decides, the category vocabulary. The seven
//! bodies below are the ones review found them disagreeing on; each must now
//! get the same verdict from `validate` and from the projection. A fully
//! populated unit passes the contract whole, which pins the contract against
//! drift.

use supercom_bp::{Key, Value};
use supercom_data::{Error, Kind, unit_from_value, validate};

fn name(key: &str, value: Value) -> (Key, Value) {
    (Key::Name(key.into()), value)
}

fn s(text: &str) -> Value {
    Value::Str(text.into())
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn table(entries: Vec<(Key, Value)>) -> Value {
    Value::Table(entries)
}

fn list(values: Vec<Value>) -> Value {
    table(
        values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (Key::Index(i), v))
            .collect(),
    )
}

/// The smallest unit body both schemas accept: one category and the
/// required economy, physics, and defense fields.
fn baseline() -> Vec<(Key, Value)> {
    vec![
        name("Categories", list(vec![s("LAND")])),
        name(
            "Economy",
            table(vec![
                name("BuildCostEnergy", num(1.0)),
                name("BuildCostMass", num(1.0)),
            ]),
        ),
        name(
            "Physics",
            table(vec![name("MotionType", s("RULEUMT_Land"))]),
        ),
        name(
            "Defense",
            table(vec![
                name("Health", num(1.0)),
                name("MaxHealth", num(1.0)),
                name("ArmorType", s("Normal")),
            ]),
        ),
        name("General", table(vec![name("CommandCaps", table(vec![]))])),
    ]
}

/// The baseline with `key` replaced (or added) by `value`.
fn with(key: &str, value: Value) -> Value {
    let mut entries: Vec<(Key, Value)> = baseline()
        .into_iter()
        .filter(|(k, _)| *k != Key::Name(key.into()))
        .collect();
    entries.push(name(key, value));
    table(entries)
}

/// The baseline with one more field inside its `Economy` table.
fn with_economy_field(key: &str, value: Value) -> Value {
    with(
        "Economy",
        table(vec![
            name("BuildCostEnergy", num(1.0)),
            name("BuildCostMass", num(1.0)),
            name(key, value),
        ]),
    )
}

/// The two verdicts, the projection's reduced to its error.
fn verdicts(body: &Value) -> (Result<(), Error>, Result<(), Error>) {
    (
        validate(Kind::Unit, body),
        unit_from_value("x", body).map(|_| ()),
    )
}

fn assert_both(body: &Value, expected: &Result<(), Error>) {
    let (contract, projection) = verdicts(body);
    assert_eq!(&contract, expected, "validate");
    assert_eq!(&projection, expected, "unit_from_value");
}

fn type_error(field: &str, expected: &str) -> Result<(), Error> {
    Err(Error::Type {
        field: field.into(),
        expected: expected.into(),
    })
}

/// An empty `Intel` table is a record with no fields, which both accept.
#[test]
fn agreement_on_an_empty_intel_table() {
    assert_both(&with("Intel", table(vec![])), &Ok(()));
}

/// A positional `Intel` table has no declared field; both accept it.
#[test]
fn agreement_on_a_positional_intel_table() {
    assert_both(&with("Intel", list(vec![num(5.0)])), &Ok(()));
}

/// A nil `BuildTime` is an absent optional field for both.
#[test]
fn agreement_on_a_nil_build_time() {
    assert_both(&with_economy_field("BuildTime", Value::Nil), &Ok(()));
}

/// A nil `VisionRadius` is an absent optional field for both.
#[test]
fn agreement_on_a_nil_vision_radius() {
    let body = with("Intel", table(vec![name("VisionRadius", Value::Nil)]));
    assert_both(&body, &Ok(()));
}

/// An infinite `BuildTime` (Lua reads `1e999` so) is not a number for
/// either, and the error names the field.
#[test]
fn agreement_on_an_infinite_build_time() {
    let body = with_economy_field("BuildTime", num(f64::INFINITY));
    assert_both(&body, &type_error("Economy.BuildTime", "Number"));
}

/// A `Weapon` table with a named key is not the array both declare.
#[test]
fn agreement_on_a_named_key_weapon_table() {
    let body = with(
        "Weapon",
        table(vec![name("a", table(vec![name("Damage", num(1.0))]))]),
    );
    assert_both(&body, &type_error("Weapon", "Array"));
}

/// A `Categories` table with a named key is not the array both declare.
#[test]
fn agreement_on_a_named_key_categories_table() {
    let body = with("Categories", table(vec![name("a", s("LAND"))]));
    assert_both(&body, &type_error("Categories", "Array"));
}

fn weapon_with(field: &str, value: Value) -> Value {
    with(
        "Weapon",
        list(vec![table(vec![
            name("Damage", num(1.0)),
            name("DamageType", s("Normal")),
            name("WeaponCategory", s("Direct Fire")),
            name(field, value),
        ])]),
    )
}

/// `ManualFire` is stored as a boolean or as the number 1; both accept
/// either, and both refuse any other number.
#[test]
fn agreement_on_a_manual_fire_of_one() {
    assert_both(&weapon_with("ManualFire", num(1.0)), &Ok(()));
    assert_both(&weapon_with("ManualFire", Value::Bool(true)), &Ok(()));
    assert_both(
        &weapon_with("ManualFire", num(2.0)),
        &type_error("Weapon[0].ManualFire", "Boolean"),
    );
}

/// A `CommandCaps` value that is not a boolean is refused by both.
#[test]
fn agreement_on_a_non_boolean_command_cap() {
    let caps = table(vec![name("RULEUCC_Move", s("yes"))]);
    let body = with("General", table(vec![name("CommandCaps", caps)]));
    assert_both(
        &body,
        &type_error("General.CommandCaps.RULEUCC_Move", "Boolean"),
    );
}

/// A `Footprint` without `SizeX` is accepted by both: the field is
/// optional.
#[test]
fn agreement_on_a_footprint_without_size_x() {
    let body = with("Footprint", table(vec![name("SizeZ", num(2.0))]));
    assert_both(&body, &Ok(()));
    let unit = unit_from_value("x", &body).map(|u| u.footprint.map(|f| (f.size_x, f.size_z)));
    assert_eq!(unit, Ok(Some((None, Some(2.0)))));
}

/// A `BuildableCategory` that is a string, not a table, is refused by both.
#[test]
fn agreement_on_a_string_buildable_category() {
    let body = with_economy_field("BuildableCategory", s("ENGINEER"));
    assert_both(&body, &type_error("Economy.BuildableCategory", "Array"));
}

/// A `TargetPriorities` entry that is not a string is refused by both.
#[test]
fn agreement_on_a_numeric_target_priority() {
    let body = weapon_with("TargetPriorities", list(vec![num(3.0)]));
    assert_both(
        &body,
        &type_error("Weapon[0].TargetPriorities[0]", "String"),
    );
}

/// A unit carrying every declared field passes the contract whole.
#[test]
fn a_fully_populated_unit_validates() {
    let weapon = |label: &str| {
        table(vec![
            name("Label", s(label)),
            name("Damage", num(10.0)),
            name("DamageType", s("Normal")),
            name("WeaponCategory", s("Direct Fire")),
            name("RateOfFire", num(0.5)),
            name("MaxRadius", num(30.0)),
            name("ProjectileId", s("/projectiles/x/x_proj.bp")),
            name("DamageRadius", num(2.0)),
            name("DamageFriendly", Value::Bool(true)),
            name("MinRadius", num(1.0)),
            name("MuzzleVelocity", num(20.0)),
            name("ProjectilesPerOnFire", num(1.0)),
            name("MuzzleSalvoSize", num(1.0)),
            name("MuzzleSalvoDelay", num(0.1)),
            name("RackSalvoReloadTime", num(2.0)),
            name("RackSalvoChargeTime", num(0.5)),
            name("RackSalvoFiresAfterCharge", Value::Bool(false)),
            name("BallisticArc", s("RULEUBA_LowArc")),
            name("TargetPriorities", list(vec![s("COMMAND")])),
            name(
                "FireTargetLayerCapsTable",
                table(vec![name("Land", s("Land|Water"))]),
            ),
            name("TrackingRadius", num(1.0)),
            name("FiringTolerance", num(1.0)),
            name("TargetCheckInterval", num(0.5)),
            name("Turreted", Value::Bool(true)),
            name("ManualFire", num(1.0)),
        ])
    };
    let body = table(vec![
        name("Categories", list(vec![s("LAND"), s("MOBILE")])),
        name(
            "Economy",
            table(vec![
                name("BuildCostEnergy", num(100.0)),
                name("BuildCostMass", num(10.0)),
                name("BuildTime", num(50.0)),
                name("BuildRate", num(5.0)),
                name("ProductionPerSecondEnergy", num(1.0)),
                name("ProductionPerSecondMass", num(0.1)),
                name("StorageEnergy", num(500.0)),
                name("StorageMass", num(50.0)),
                name("BuildableCategory", list(vec![s("ENGINEER * LAND")])),
                name("MaintenanceConsumptionPerSecondEnergy", num(5.0)),
            ]),
        ),
        name(
            "Physics",
            table(vec![
                name("MotionType", s("RULEUMT_Land")),
                name("MaxSpeed", num(3.0)),
                name("TurnRate", num(90.0)),
                name("MaxAcceleration", num(2.0)),
                name("MaxBrake", num(3.0)),
                name("Elevation", num(4.0)),
                name("MinSpeedPercent", num(0.5)),
                name("BuildRestriction", s("RULEUBR_OnMassDeposit")),
            ]),
        ),
        name(
            "Defense",
            table(vec![
                name("Health", num(200.0)),
                name("MaxHealth", num(200.0)),
                name("ArmorType", s("Normal")),
                name("RegenRate", num(1.0)),
            ]),
        ),
        name(
            "Intel",
            table(vec![
                name("VisionRadius", num(20.0)),
                name("RadarRadius", num(40.0)),
            ]),
        ),
        name("Weapon", list(vec![weapon("Gun"), weapon("Missile")])),
        name(
            "General",
            table(vec![name(
                "CommandCaps",
                table(vec![name("RULEUCC_Move", Value::Bool(true))]),
            )]),
        ),
        name(
            "Footprint",
            table(vec![
                name("SizeX", num(1.0)),
                name("SizeZ", num(1.0)),
                name("MinWaterDepth", num(0.0)),
            ]),
        ),
    ]);
    assert_both(&body, &Ok(()));
}

/// `ManualFire` of 2 is refused by both, with the same path.
#[test]
fn agreement_on_a_manual_fire_of_two() {
    assert_both(
        &weapon_with("ManualFire", num(2.0)),
        &type_error("Weapon[0].ManualFire", "Boolean"),
    );
}

/// A string `ManualFire` is refused by both, with the same path.
#[test]
fn agreement_on_a_string_manual_fire() {
    assert_both(
        &weapon_with("ManualFire", s("yes")),
        &type_error("Weapon[0].ManualFire", "Boolean"),
    );
}

/// A `BuildOnLayerCaps` value that is not a boolean is refused by both.
#[test]
fn agreement_on_a_non_boolean_build_on_layer_cap() {
    let physics = table(vec![
        name("MotionType", s("RULEUMT_Land")),
        name(
            "BuildOnLayerCaps",
            table(vec![name("LAYER_Land", s("yes"))]),
        ),
    ]);
    assert_both(
        &with("Physics", physics),
        &type_error("Physics.BuildOnLayerCaps.LAYER_Land", "Boolean"),
    );
}

/// A projectile with a mistyped `Physics` field gets the same verdict
/// from `validate` as from the projection.
#[test]
fn agreement_on_a_mistyped_projectile_physics_field() {
    let body = table(vec![name(
        "Physics",
        table(vec![name("TrackTarget", num(1.0))]),
    )]);
    let expected = type_error("Physics.TrackTarget", "Boolean");
    assert_eq!(validate(Kind::Projectile, &body), expected);
    assert_eq!(
        supercom_data::projectile_from_value("x", &body).map(|_| ()),
        expected
    );
}

/// The ManualFire rule in one place: the boolean and the number 1 are
/// accepted, the number 2 and a string are refused, by both verdicts.
#[test]
fn manual_fire_accepts_one_and_refuses_two_and_a_string_in_both_verdicts() {
    assert_both(&weapon_with("ManualFire", num(1.0)), &Ok(()));
    for refused in [num(2.0), s("yes")] {
        assert_both(
            &weapon_with("ManualFire", refused),
            &type_error("Weapon[0].ManualFire", "Boolean"),
        );
    }
}
