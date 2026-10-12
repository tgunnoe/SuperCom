//! Synthetic suite of `supercom-data`: every body is built here as a
//! `Value` with invented identifiers and numbers; no archive and no map is
//! opened. Against the skeleton every test fails on a function returning
//! `Err(Error::Unimplemented)`.

use supercom_bp::{Key, Value};
use supercom_data::{
    Error, Footprint, Kind, projectile_from_value, prop_from_value, unit_from_value, validate,
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
        (
            "General",
            table(vec![(
                "CommandCaps",
                table(vec![("RULEUCC_Move", Value::Bool(true))]),
            )]),
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
                ("DamageType", s("Normal")),
                ("WeaponCategory", s("Direct Fire")),
                ("RateOfFire", num(2.0)),
                ("MaxRadius", num(30.0)),
                ("ProjectileId", s("/projectiles/Xyz/Xyz_proj.bp")),
            ]),
            table(vec![
                ("Damage", num(4.0)),
                ("DamageType", s("Normal")),
                ("WeaponCategory", s("Death")),
            ]),
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
    let general = |extra: bool| {
        let mut entries = vec![(
            "CommandCaps",
            table(vec![("RULEUCC_Move", Value::Bool(true))]),
        )];
        if extra {
            entries.push(("Category", s("Tank")));
        }
        table(entries)
    };
    let footprint = |extra: bool| {
        let mut entries = vec![("SizeX", num(1.0))];
        if extra {
            entries.push(("OccupyCaps", s("RULEUCC_Move")));
        }
        table(entries)
    };
    let body = |extra: bool| {
        let mut entries = minimal_unit();
        entries.retain(|(k, _)| *k != "General");
        entries.push(("General", general(extra)));
        entries.push(("Footprint", footprint(extra)));
        if extra {
            entries.push(("AnotherInvented", table(vec![("Deep", num(1.0))])));
        }
        table(entries)
    };
    let plain = unit_from_value("xyz0003", &body(false));
    let extended = unit_from_value("xyz0003", &body(true));
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
        ("Physics", table(vec![])),
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
    let bare_projectile = table(vec![("Physics", table(vec![]))]);
    for (kind, bare) in [(Kind::Projectile, &bare_projectile), (Kind::Prop, &bare)] {
        let good = if kind == Kind::Projectile {
            with(
                vec![("Categories", list(vec![s("ANY")]))],
                "Physics",
                table(vec![]),
            )
        } else {
            good.clone()
        };
        assert_eq!(validate(kind, &good), Ok(()), "{kind:?} good body");
        assert_eq!(
            validate(kind, bare),
            Ok(()),
            "{kind:?} body without Categories"
        );
    }
    let projectile =
        projectile_from_value("xyz_proj", &bare_projectile).expect("projectile projects");
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

/// A call nested `depth` deep, `C(C(...C(1)...))`, built without recursion.
fn nested_call(depth: usize) -> Value {
    let mut value = num(1.0);
    for _ in 0..depth {
        value = Value::Call {
            name: "C".to_owned(),
            args: vec![value],
        };
    }
    value
}

/// A unit body whose `Economy` carries the undeclared field `Z` holding
/// `extra`.
fn unit_with_economy_field(name: &str, extra: Value) -> Value {
    let mut entries = minimal_unit();
    entries.retain(|(k, _)| *k != "Economy");
    let economy = Value::Table(vec![
        (Key::Name("BuildCostEnergy".to_owned()), num(1.0)),
        (Key::Name("BuildCostMass".to_owned()), num(1.0)),
        (Key::Name(name.to_owned()), extra),
    ]);
    let mut body = table(entries);
    if let Value::Table(rows) = &mut body {
        rows.push((Key::Name("Economy".to_owned()), economy));
    }
    body
}

/// A field no contract declares never reaches Nickel: a 200-deep nested call
/// in an undeclared `Economy` field validates on a 2 MiB thread, where the
/// whole subtree rendered into the evaluator once overflowed its stack and
/// aborted the process.
#[test]
fn deep_nesting_in_an_undeclared_field_does_not_abort() {
    let body = unit_with_economy_field("Z", nested_call(200));
    let result = std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || validate(Kind::Unit, &body))
        .expect("spawn")
        .join()
        .expect("validate must not panic");
    assert_eq!(result, Ok(()));
}

/// Crafted strings stay data: a category that tries to interpolate or close
/// the string is a vocabulary error, never an import or evaluation error,
/// and crafted field names in a declared block never change the verdict.
#[test]
fn injection_attempts_in_strings_and_field_names_stay_data() {
    let crafted = [
        "%{ import \"/nonexistent\" }",
        "\" | { x = import \"/nonexistent\" } + \"",
        "\nimport \"/nonexistent\"",
        "\\\" } + import \"/nonexistent\"",
        "\u{0}\u{1}\u{7f}\u{c8}\u{ff}",
    ];
    for text in crafted {
        let mut entries = minimal_unit();
        entries.retain(|(k, _)| *k != "Categories");
        entries.push(("Categories", list(vec![s(text)])));
        let result = validate(Kind::Unit, &table(entries));
        assert!(
            matches!(&result, Err(Error::Contract { path, reason })
                if path.starts_with("Categories") && !reason.contains("import")),
            "expected a vocabulary Contract error for {text:?}, got {result:?}"
        );
        let body = unit_with_economy_field(text, s(text));
        assert_eq!(validate(Kind::Unit, &body), Ok(()), "field name {text:?}");
    }
}

fn flag(b: bool) -> Value {
    Value::Bool(b)
}

/// The minimal unit with `key` replaced by `value`.
fn minimal_with(key: &'static str, value: Value) -> Value {
    with(minimal_unit(), key, value)
}

/// A weapon table holding only the required fields plus `extra`.
fn weapon(extra: Vec<(&'static str, Value)>) -> Value {
    let mut entries = vec![
        ("Damage", num(5.0)),
        ("DamageType", s("Normal")),
        ("WeaponCategory", s("Direct Fire")),
    ];
    entries.extend(extra);
    table(entries)
}

/// The minimal unit with one weapon holding `extra`.
fn unit_with_weapon(extra: Vec<(&'static str, Value)>) -> Value {
    minimal_with("Weapon", list(vec![weapon(extra)]))
}

/// Pins the economy additions: the build category expressions read as
/// stored, in order, and the maintenance drain reads through; both are
/// empty or `None` when the body omits them.
#[test]
fn economy_build_fields_project() {
    let economy = table(vec![
        ("BuildCostEnergy", num(1.0)),
        ("BuildCostMass", num(2.0)),
        (
            "BuildableCategory",
            list(vec![s("BUILTBYTIER1 * LAND"), s("ENGINEER + MOBILE")]),
        ),
        ("MaintenanceConsumptionPerSecondEnergy", num(75.0)),
    ]);
    let unit = unit_from_value("xyz0010", &minimal_with("Economy", economy));
    let unit = unit.expect("must project");
    assert_eq!(
        unit.economy.buildable_category,
        ["BUILTBYTIER1 * LAND", "ENGINEER + MOBILE"]
    );
    assert_eq!(
        unit.economy.maintenance_consumption_per_second_energy,
        Some(75.0)
    );
    let bare = unit_from_value("xyz0011", &table(minimal_unit())).expect("must project");
    assert_eq!(bare.economy.buildable_category, Vec::<String>::new());
    assert_eq!(bare.economy.maintenance_consumption_per_second_energy, None);
}

/// Pins the physics additions, including the build restriction string.
#[test]
fn physics_movement_and_placement_fields_project() {
    let physics = table(vec![
        ("MotionType", s("RULEUMT_Land")),
        ("MaxAcceleration", num(2.5)),
        ("MaxBrake", num(3.5)),
        ("Elevation", num(-4.0)),
        ("MinSpeedPercent", num(0.25)),
        ("BuildRestriction", s("RULEUBR_OnMassDeposit")),
    ]);
    let unit = unit_from_value("xyz0012", &minimal_with("Physics", physics)).expect("must project");
    assert_eq!(unit.physics.max_acceleration, Some(2.5));
    assert_eq!(unit.physics.max_brake, Some(3.5));
    assert_eq!(unit.physics.elevation, Some(-4.0));
    assert_eq!(unit.physics.min_speed_percent, Some(0.25));
    assert_eq!(
        unit.physics.build_restriction.as_deref(),
        Some("RULEUBR_OnMassDeposit")
    );
}

/// Pins the footprint: absent table is `None`; a present one reads its four
/// fields, each `None` when omitted (never a default of 1).
#[test]
fn footprint_is_none_when_absent_and_partial_when_present() {
    let none = unit_from_value("xyz0013", &table(minimal_unit())).expect("must project");
    assert_eq!(none.footprint, None);
    let footprint = table(vec![("SizeZ", num(3.0)), ("MinWaterDepth", num(1.5))]);
    let unit = unit_from_value("xyz0014", &minimal_with("Footprint", footprint)).expect("projects");
    assert_eq!(
        unit.footprint,
        Some(Footprint {
            size_x: None,
            size_z: Some(3.0),
            min_water_depth: Some(1.5),
        })
    );
}

/// The command caps of the unit are exactly the true keys, sorted
/// ascending, whatever the order the table stores them in: checked over
/// every ordering of four keys and every assignment of true and false.
#[test]
fn command_caps_are_the_sorted_true_keys_for_every_order_and_assignment() {
    const KEYS: [&str; 4] = [
        "RULEUCC_Stop",
        "RULEUCC_Attack",
        "RULEUCC_Move",
        "RULEUCC_Guard",
    ];
    let mut orders: Vec<Vec<usize>> = Vec::new();
    for a in 0..4 {
        for b in (0..4).filter(|b| *b != a) {
            for c in (0..4).filter(|c| *c != a && *c != b) {
                let d = 6 - a - b - c;
                orders.push(vec![a, b, c, d]);
            }
        }
    }
    assert_eq!(orders.len(), 24);
    let mut checked = 0;
    for order in &orders {
        for mask in 0..16_u32 {
            let entries: Vec<(&str, Value)> = order
                .iter()
                .map(|&i| (KEYS[i], flag(mask >> i & 1 == 1)))
                .collect();
            let general = table(vec![("CommandCaps", table(entries))]);
            let unit = unit_from_value("xyz0015", &minimal_with("General", general));
            let unit = unit.expect("must project");
            let mut expected: Vec<String> = (0..4)
                .filter(|i| mask >> i & 1 == 1)
                .map(|i| KEYS[i].to_owned())
                .collect();
            expected.sort();
            assert_eq!(unit.command_caps, expected, "order {order:?} mask {mask}");
            checked += 1;
        }
    }
    assert_eq!(checked, 384);
}

/// `General` is required, as is the `CommandCaps` table inside it.
#[test]
fn a_unit_without_command_caps_is_missing_them() {
    let without_general = without(minimal_unit(), "General");
    assert_eq!(
        unit_from_value("xyz0016", &without_general),
        Err(Error::Missing {
            field: "General".into()
        })
    );
    let without_caps = minimal_with("General", table(vec![]));
    assert_eq!(
        unit_from_value("xyz0017", &without_caps),
        Err(Error::Missing {
            field: "General.CommandCaps".into()
        })
    );
}

/// Pins the weapon additions on a weapon holding every one of them.
#[test]
fn weapon_additions_project() {
    let body = unit_with_weapon(vec![
        ("DamageRadius", num(2.5)),
        ("DamageFriendly", flag(true)),
        ("MinRadius", num(4.0)),
        ("MuzzleVelocity", num(30.0)),
        ("ProjectilesPerOnFire", num(3.0)),
        ("MuzzleSalvoSize", num(2.0)),
        ("MuzzleSalvoDelay", num(0.5)),
        ("RackSalvoReloadTime", num(6.0)),
        ("RackSalvoChargeTime", num(1.0)),
        ("RackSalvoFiresAfterCharge", flag(false)),
        ("BallisticArc", s("RULEUBA_HighArc")),
        (
            "TargetPriorities",
            list(vec![s("COMMAND"), s("MOBILE - AIR")]),
        ),
        (
            "FireTargetLayerCapsTable",
            table(vec![("Water", s("Land|Water")), ("Air", s("Air"))]),
        ),
        ("TrackingRadius", num(1.2)),
        ("FiringTolerance", num(2.0)),
        ("TargetCheckInterval", num(0.5)),
        ("Turreted", flag(true)),
        ("ManualFire", flag(false)),
    ]);
    let unit = unit_from_value("xyz0018", &body).expect("must project");
    let w = &unit.weapons[0];
    assert_eq!(w.damage_type, "Normal");
    assert_eq!(w.weapon_category, "Direct Fire");
    assert_eq!(w.damage_radius, Some(2.5));
    assert_eq!(w.damage_friendly, Some(true));
    assert_eq!(w.min_radius, Some(4.0));
    assert_eq!(w.muzzle_velocity, Some(30.0));
    assert_eq!(w.projectiles_per_on_fire, Some(3.0));
    assert_eq!(w.muzzle_salvo_size, Some(2.0));
    assert_eq!(w.muzzle_salvo_delay, Some(0.5));
    assert_eq!(w.rack_salvo_reload_time, Some(6.0));
    assert_eq!(w.rack_salvo_charge_time, Some(1.0));
    assert_eq!(w.rack_salvo_fires_after_charge, Some(false));
    assert_eq!(w.ballistic_arc.as_deref(), Some("RULEUBA_HighArc"));
    assert_eq!(w.target_priorities, ["COMMAND", "MOBILE - AIR"]);
    assert_eq!(
        w.fire_target_layer_caps,
        [
            ("Air".to_owned(), "Air".to_owned()),
            ("Water".to_owned(), "Land|Water".to_owned())
        ]
    );
    assert_eq!(w.tracking_radius, Some(1.2));
    assert_eq!(w.firing_tolerance, Some(2.0));
    assert_eq!(w.target_check_interval, Some(0.5));
    assert_eq!(w.turreted, Some(true));
    assert_eq!(w.manual_fire, Some(false));
}

/// A weapon that omits the optional additions reads them as `None` or
/// empty; the required `DamageType` and `WeaponCategory` read through.
#[test]
fn a_weapon_without_the_optional_additions_reads_them_as_absent() {
    let unit = unit_from_value("xyz0019", &unit_with_weapon(vec![])).expect("must project");
    let w = &unit.weapons[0];
    assert_eq!(w.damage_type, "Normal");
    assert_eq!(w.weapon_category, "Direct Fire");
    assert_eq!(w.damage_radius, None);
    assert_eq!(w.damage_friendly, None);
    assert_eq!(w.ballistic_arc, None);
    assert_eq!(w.target_priorities, Vec::<String>::new());
    assert_eq!(w.fire_target_layer_caps, Vec::<(String, String)>::new());
    assert_eq!(w.turreted, None);
    assert_eq!(w.manual_fire, None);
}

/// `ManualFire` is stored as a boolean or as the number 1, which reads as
/// true.
#[test]
fn manual_fire_reads_a_boolean_or_the_number_one() {
    for (stored, expected) in [(flag(true), true), (flag(false), false), (num(1.0), true)] {
        let body = unit_with_weapon(vec![("ManualFire", stored.clone())]);
        let unit = unit_from_value("xyz0020", &body).expect("must project");
        assert_eq!(unit.weapons[0].manual_fire, Some(expected), "{stored:?}");
    }
}

/// `DamageType` and `WeaponCategory` are required of every weapon, so
/// their absence is named by weapon index.
#[test]
fn a_weapon_without_damage_type_or_category_is_missing_it() {
    let no_type = minimal_with(
        "Weapon",
        list(vec![table(vec![
            ("Damage", num(1.0)),
            ("WeaponCategory", s("Death")),
        ])]),
    );
    assert_eq!(
        unit_from_value("xyz0021", &no_type),
        Err(Error::Missing {
            field: "Weapon[0].DamageType".into()
        })
    );
    let no_category = minimal_with(
        "Weapon",
        list(vec![table(vec![
            ("Damage", num(1.0)),
            ("DamageType", s("Normal")),
        ])]),
    );
    assert_eq!(
        unit_from_value("xyz0022", &no_category),
        Err(Error::Missing {
            field: "Weapon[0].WeaponCategory".into()
        })
    );
}

/// A mistyped additional field is refused with its path, by the projection
/// and by `validate` alike.
#[test]
fn mistyped_additional_fields_name_their_path() {
    let elevation = minimal_with(
        "Physics",
        table(vec![
            ("MotionType", s("RULEUMT_Air")),
            ("Elevation", s("high")),
        ]),
    );
    let expected = Err(Error::Type {
        field: "Physics.Elevation".into(),
        expected: "Number".into(),
    });
    assert_eq!(unit_from_value("xyz0023", &elevation).map(|_| ()), expected);
    assert_eq!(validate(Kind::Unit, &elevation), expected);
    let turreted = unit_with_weapon(vec![("Turreted", num(1.0))]);
    let expected = Err(Error::Type {
        field: "Weapon[0].Turreted".into(),
        expected: "Boolean".into(),
    });
    assert_eq!(unit_from_value("xyz0024", &turreted).map(|_| ()), expected);
    assert_eq!(validate(Kind::Unit, &turreted), expected);
}

/// Pins the projectile physics: every field reads through, and a field
/// outside the declared set is ignored.
#[test]
fn projectile_physics_projects_and_ignores_undeclared_fields() {
    let body = table(vec![(
        "Physics",
        table(vec![
            ("InitialSpeed", num(12.0)),
            ("MaxSpeed", num(40.0)),
            ("Acceleration", num(-0.4)),
            ("TurnRate", num(180.0)),
            ("Lifetime", num(7.5)),
            ("TrackTarget", flag(true)),
            ("UseGravity", flag(false)),
            ("LeadTarget", flag(true)),
            ("VelocityAlign", flag(false)),
            ("DestroyOnWater", flag(true)),
            ("InventedField", num(9.0)),
        ]),
    )]);
    let projectile = projectile_from_value("/projectiles/xyz/xyz_proj.bp", &body);
    let p = projectile.expect("must project").physics;
    assert_eq!(p.initial_speed, Some(12.0));
    assert_eq!(p.max_speed, Some(40.0));
    assert_eq!(p.acceleration, Some(-0.4));
    assert_eq!(p.turn_rate, Some(180.0));
    assert_eq!(p.lifetime, Some(7.5));
    assert_eq!(p.track_target, Some(true));
    assert_eq!(p.use_gravity, Some(false));
    assert_eq!(p.lead_target, Some(true));
    assert_eq!(p.velocity_align, Some(false));
    assert_eq!(p.destroy_on_water, Some(true));
}

/// The projectile `Physics` table is required, as all 365 installed
/// projectiles carry it; an empty one reads every field as `None`.
#[test]
fn projectile_physics_is_required_and_may_be_empty() {
    assert_eq!(
        projectile_from_value("x", &table(vec![("Other", num(1.0))])).map(|_| ()),
        Err(Error::Missing {
            field: "Physics".into()
        })
    );
    let empty = table(vec![("Physics", table(vec![]))]);
    let p = projectile_from_value("x", &empty)
        .expect("must project")
        .physics;
    assert_eq!(p.initial_speed, None);
    assert_eq!(p.destroy_on_water, None);
}

/// Metamorphic: fields no contract names, added inside the new blocks and
/// at the top level, leave the projection of a fully populated unit
/// unchanged (the outside-the-set rule extended to the new blocks).
#[test]
fn projection_ignores_undeclared_fields_inside_the_new_blocks() {
    let build = |extra: bool| {
        let mut physics = vec![("MotionType", s("RULEUMT_Land")), ("Elevation", num(1.0))];
        let mut footprint = vec![("SizeX", num(2.0)), ("SizeZ", num(2.0))];
        let mut general = vec![("CommandCaps", table(vec![("RULEUCC_Move", flag(true))]))];
        let mut w = vec![("TrackingRadius", num(1.0))];
        let top = if extra {
            vec![("Shield", table(vec![("Depth", num(1.0))]))]
        } else {
            vec![]
        };
        if extra {
            physics.push(("MotionLayer", s("Land")));
            footprint.push(("OccupyCaps", s("RULEUCC_Move")));
            general.push(("Category", s("Tank")));
            w.push(("RackBones", table(vec![("Bone", s("b"))])));
        }
        let mut entries = minimal_unit();
        entries.retain(|(k, _)| !["Physics", "General"].contains(k));
        entries.push(("Physics", table(physics)));
        entries.push(("General", table(general)));
        entries.push(("Footprint", table(footprint)));
        entries.push(("Weapon", list(vec![weapon(w)])));
        entries.extend(top);
        unit_from_value("xyz0025", &table(entries))
    };
    let plain = build(false);
    assert!(plain.is_ok(), "the plain body must project, got {plain:?}");
    assert_eq!(
        plain.as_ref().map(|u| u.command_caps.clone()),
        Ok(vec!["RULEUCC_Move".to_owned()])
    );
    assert_eq!(plain, build(true));
}

/// `validate` runs the projection before the contract for a projectile, as
/// for a unit, so a missing `Physics` is `Missing` at that path rather than
/// a contract blame attributed to `Categories`.
#[test]
fn validate_reports_a_projectile_without_physics_as_missing() {
    assert_eq!(
        validate(Kind::Projectile, &table(vec![("Other", num(1.0))])),
        Err(Error::Missing {
            field: "Physics".into()
        })
    );
}

/// A mistyped projectile `Physics` field is a `Type` error at its path
/// from `validate`, not a contract blame.
#[test]
fn validate_reports_a_mistyped_projectile_physics_field_at_its_path() {
    let body = table(vec![("Physics", table(vec![("TrackTarget", num(1.0))]))]);
    assert_eq!(
        validate(Kind::Projectile, &body),
        Err(Error::Type {
            field: "Physics.TrackTarget".into(),
            expected: "Boolean".into()
        })
    );
}

/// A projectile carrying every declared field validates.
#[test]
fn a_fully_populated_projectile_validates() {
    let body = table(vec![
        ("Categories", list(vec![s("PROJECTILE")])),
        (
            "Physics",
            table(vec![
                ("InitialSpeed", num(1.0)),
                ("MaxSpeed", num(2.0)),
                ("Acceleration", num(3.0)),
                ("TurnRate", num(4.0)),
                ("Lifetime", num(5.0)),
                ("TrackTarget", flag(true)),
                ("UseGravity", flag(false)),
                ("LeadTarget", flag(true)),
                ("VelocityAlign", flag(false)),
                ("DestroyOnWater", flag(true)),
            ]),
        ),
    ]);
    assert_eq!(validate(Kind::Projectile, &body), Ok(()));
}

/// The layer capabilities are the true keys of the table, sorted
/// ascending whatever the stored order, and empty when the table is absent.
#[test]
fn build_on_layer_caps_are_the_sorted_true_keys_and_empty_when_absent() {
    let stored = table(vec![
        ("LAYER_Water", flag(true)),
        ("LAYER_Air", flag(false)),
        ("LAYER_Land", flag(true)),
        ("LAYER_Orbit", flag(false)),
        ("LAYER_Seabed", flag(true)),
    ]);
    let physics = table(vec![
        ("MotionType", s("RULEUMT_Land")),
        ("BuildOnLayerCaps", stored),
    ]);
    let unit = unit_from_value("xyz0030", &minimal_with("Physics", physics)).expect("must project");
    assert_eq!(
        unit.physics.build_on_layer_caps,
        ["LAYER_Land", "LAYER_Seabed", "LAYER_Water"]
    );
    let bare = unit_from_value("xyz0031", &table(minimal_unit())).expect("projects");
    assert_eq!(bare.physics.build_on_layer_caps, Vec::<String>::new());
}

/// A layer capability that is not a boolean is a `Type` error at its path.
#[test]
fn a_non_boolean_build_on_layer_cap_names_its_path() {
    let physics = table(vec![
        ("MotionType", s("RULEUMT_Land")),
        ("BuildOnLayerCaps", table(vec![("LAYER_Land", s("yes"))])),
    ]);
    assert_eq!(
        unit_from_value("x", &minimal_with("Physics", physics)).map(|_| ()),
        Err(Error::Type {
            field: "Physics.BuildOnLayerCaps.LAYER_Land".into(),
            expected: "Boolean".into()
        })
    );
}
