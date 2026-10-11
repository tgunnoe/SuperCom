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

/// A unit carrying every declared field passes the contract whole.
#[test]
fn a_fully_populated_unit_validates() {
    let weapon = |label: &str| {
        table(vec![
            name("Label", s(label)),
            name("Damage", num(10.0)),
            name("RateOfFire", num(0.5)),
            name("MaxRadius", num(30.0)),
            name("ProjectileId", s("/projectiles/x/x_proj.bp")),
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
            ]),
        ),
        name(
            "Physics",
            table(vec![
                name("MotionType", s("RULEUMT_Land")),
                name("MaxSpeed", num(3.0)),
                name("TurnRate", num(90.0)),
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
    ]);
    assert_both(&body, &Ok(()));
}
