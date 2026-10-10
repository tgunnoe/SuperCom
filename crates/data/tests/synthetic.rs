//! Synthetic suite of `supercom-data`: every body is built here as a
//! `Value` with invented identifiers and numbers; no archive and no map is
//! opened. Against the skeleton every test fails on a function returning
//! `Err(Error::Unimplemented)`.

use supercom_bp::{Key, Value};
use supercom_data::{
    Error, Kind, projectile_from_value, prop_from_value, unit_from_value, validate,
};

fn table(entries: Vec<(&str, Value)>) -> Value {
    Value::Table(
        entries
            .into_iter()
            .map(|(k, v)| (Key::Name(k.to_owned()), v))
            .collect(),
    )
}

fn list(items: Vec<Value>) -> Value {
    Value::Table(
        items
            .into_iter()
            .enumerate()
            .map(|(i, v)| (Key::Index(i + 1), v))
            .collect(),
    )
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn s(text: &str) -> Value {
    Value::Str(text.to_owned())
}

/// A minimal valid unit body: only the fields the installed data always
/// carries (every optional field omitted), plus one field no contract names.
fn minimal_unit() -> Vec<(&'static str, Value)> {
    vec![
        ("Categories", list(vec![s("AIR"), s("MOBILE")])),
        (
            "Economy",
            table(vec![
                ("BuildCostEnergy", num(120.0)),
                ("BuildCostMass", num(30.0)),
            ]),
        ),
        ("Physics", table(vec![("MotionType", s("RULEUMT_Air"))])),
        (
            "Defense",
            table(vec![
                ("Health", num(50.0)),
                ("MaxHealth", num(50.0)),
                ("ArmorType", s("Normal")),
            ]),
        ),
        ("InventedField", s("left alone")),
    ]
}

/// Replaces `key` in `entries` with `value`, or appends it.
fn with(mut entries: Vec<(&'static str, Value)>, key: &'static str, value: Value) -> Value {
    entries.retain(|(k, _)| *k != key);
    entries.push((key, value));
    table(entries)
}

/// Removes `key` from `entries`.
fn without(mut entries: Vec<(&'static str, Value)>, key: &str) -> Value {
    entries.retain(|(k, _)| *k != key);
    table(entries)
}

/// Pins the projection of a minimal unit: the id and categories carry over,
/// the required numbers read through, and every optional field the body
/// omits is `None` or empty, never an invented default.
#[test]
fn minimal_unit_projects_with_absent_fields_as_none() {
    let unit = unit_from_value("xyz0001", &table(minimal_unit()));
    let unit = unit.expect("a minimal unit must project");
    assert_eq!(unit.id, "xyz0001");
    assert_eq!(unit.categories, ["AIR", "MOBILE"]);
    assert_eq!(unit.economy.build_cost_energy, 120.0);
    assert_eq!(unit.economy.build_cost_mass, 30.0);
    assert_eq!(unit.economy.build_time, None);
    assert_eq!(unit.economy.storage_mass, None);
    assert_eq!(unit.physics.motion_type, "RULEUMT_Air");
    assert_eq!(unit.physics.max_speed, None);
    assert_eq!(unit.defense.armor_type, "Normal");
    assert_eq!(unit.defense.regen_rate, None);
    assert_eq!(unit.intel, None);
    assert_eq!(unit.weapons.len(), 0);
}

/// Pins the projection of the optional blocks: a present `BuildTime`,
/// `Intel` block with one field omitted, and a weapon table read through,
/// with the weapon's omitted `RateOfFire` and `MaxRadius` as `None`.
#[test]
fn optional_blocks_and_weapons_project() {
    let mut entries = minimal_unit();
    entries.push(("Intel", table(vec![("RadarRadius", num(90.0))])));
    entries.push((
        "Weapon",
        list(vec![
            table(vec![
                ("Label", s("Gun")),
                ("Damage", num(12.5)),
                ("RateOfFire", num(2.0)),
                ("MaxRadius", num(30.0)),
                ("ProjectileId", s("/projectiles/Xyz/Xyz_proj.bp")),
            ]),
            table(vec![("Damage", num(4.0))]),
        ]),
    ));
    let body = with(
        entries,
        "Economy",
        table(vec![
            ("BuildCostEnergy", num(1.0)),
            ("BuildCostMass", num(2.0)),
            ("BuildTime", num(300.0)),
        ]),
    );
    let unit = unit_from_value("xyz0002", &body).expect("must project");
    assert_eq!(unit.economy.build_time, Some(300.0));
    let intel = unit.intel.expect("intel present");
    assert_eq!(intel.vision_radius, None);
    assert_eq!(intel.radar_radius, Some(90.0));
    assert_eq!(unit.weapons.len(), 2);
    assert_eq!(unit.weapons[0].label.as_deref(), Some("Gun"));
    assert_eq!(unit.weapons[0].rate_of_fire, Some(2.0));
    assert_eq!(
        unit.weapons[0].projectile_id.as_deref(),
        Some("/projectiles/Xyz/Xyz_proj.bp")
    );
    assert_eq!(unit.weapons[1].label, None);
    assert_eq!(unit.weapons[1].damage, 4.0);
    assert_eq!(unit.weapons[1].rate_of_fire, None);
    assert_eq!(unit.weapons[1].max_radius, None);
}

/// Pins the metamorphic relation of the projection: adding a field no
/// contract names does not change the projected unit.
#[test]
fn projection_ignores_fields_outside_the_v0_set() {
    let plain = unit_from_value("xyz0003", &table(minimal_unit()));
    let mut more = minimal_unit();
    more.push(("AnotherInvented", table(vec![("Deep", num(1.0))])));
    let extended = unit_from_value("xyz0003", &table(more));
    assert!(plain.is_ok(), "the plain body must project, got {plain:?}");
    assert_eq!(plain, extended);
}

/// Pins validation of a good body: the minimal unit, with an extra field the
/// contract leaves open, passes the unit contract.
#[test]
fn valid_unit_passes_its_contract() {
    let result = validate(Kind::Unit, &table(minimal_unit()));
    assert_eq!(result, Ok(()));
}

/// Pins the missing-field rule: a unit body without `Economy` fails
/// validation with `Missing { field: "Economy" }`, and the same removal from
/// the projection reports the same field.
#[test]
fn unit_missing_economy_is_missing() {
    let body = without(minimal_unit(), "Economy");
    assert_eq!(
        validate(Kind::Unit, &body),
        Err(Error::Missing {
            field: "Economy".into()
        })
    );
    assert_eq!(
        unit_from_value("xyz0004", &body),
        Err(Error::Missing {
            field: "Economy".into()
        })
    );
}

/// Pins the type rule: a string where `BuildCostMass` wants a number is a
/// `Type` error naming that field, from validation and from the projection.
#[test]
fn wrong_type_reports_type() {
    let economy = table(vec![
        ("BuildCostEnergy", num(1.0)),
        ("BuildCostMass", s("not a number")),
    ]);
    let body = with(minimal_unit(), "Economy", economy);
    let validated = validate(Kind::Unit, &body);
    assert!(
        matches!(&validated, Err(Error::Type { field, .. }) if field.ends_with("BuildCostMass")),
        "expected Type naming BuildCostMass, got {validated:?}"
    );
    let projected = unit_from_value("xyz0005", &body);
    assert!(
        matches!(&projected, Err(Error::Type { field, .. }) if field.ends_with("BuildCostMass")),
        "expected Type naming BuildCostMass, got {projected:?}"
    );
}

/// Pins the vocabulary: a unit whose category is not in the committed
/// vocabulary fails the unit contract with `Contract`.
#[test]
fn unknown_unit_category_violates_the_contract() {
    let body = with(
        minimal_unit(),
        "Categories",
        list(vec![s("NOT_A_CATEGORY_XYZ")]),
    );
    let result = validate(Kind::Unit, &body);
    assert!(
        matches!(result, Err(Error::Contract { .. })),
        "expected Contract, got {result:?}"
    );
}

/// Pins the projectile and prop projections: id and categories carry over,
/// every other field is ignored.
#[test]
fn projectile_and_prop_project_id_and_categories() {
    let body = table(vec![
        ("Categories", list(vec![s("PROJECTILE"), s("DIRECTFIRE")])),
        ("Other", num(1.0)),
    ]);
    let projectile = projectile_from_value("xyz_proj", &body).expect("projectile projects");
    assert_eq!(projectile.id, "xyz_proj");
    assert_eq!(projectile.categories, ["PROJECTILE", "DIRECTFIRE"]);
    let prop = prop_from_value("xyz_prop", &body).expect("prop projects");
    assert_eq!(prop.id, "xyz_prop");
    assert_eq!(prop.categories, ["PROJECTILE", "DIRECTFIRE"]);
}

/// Pins the projectile and prop contracts: a body with a `Categories` list
/// passes `Kind::Projectile` and `Kind::Prop`, and one without it passes
/// too and projects to an empty category list (90 of 365 installed
/// projectiles and 37 of 336 props carry no `Categories` table).
#[test]
fn projectile_and_prop_contracts_allow_absent_categories() {
    let good = table(vec![("Categories", list(vec![s("ANY")]))]);
    let bare = table(vec![("Other", num(1.0))]);
    for kind in [Kind::Projectile, Kind::Prop] {
        assert_eq!(validate(kind, &good), Ok(()), "{kind:?} good body");
        assert_eq!(
            validate(kind, &bare),
            Ok(()),
            "{kind:?} body without Categories"
        );
    }
    let projectile = projectile_from_value("xyz_proj", &bare).expect("projectile projects");
    assert_eq!(projectile.categories, Vec::<String>::new());
    let prop = prop_from_value("xyz_prop", &bare).expect("prop projects");
    assert_eq!(prop.categories, Vec::<String>::new());
}

/// Pins the unit contract: unlike projectiles and props, a unit body
/// without `Categories` is `Missing { field: "Categories" }`.
#[test]
fn unit_contract_requires_categories() {
    let body = without(minimal_unit(), "Categories");
    let missing = Err(Error::Missing {
        field: "Categories".into(),
    });
    assert_eq!(validate(Kind::Unit, &body), missing);
    assert_eq!(unit_from_value("xyz0006", &body).map(|_| ()), missing);
}
