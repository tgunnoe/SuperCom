//! The typed v0 model of unit, projectile, and prop blueprints, and the
//! Nickel contracts that validate them.
//!
//! The contracts live in `nickel/`; [`validate`] applies one in-process
//! through `nickel-lang-core`, and the `*_from_value` functions project the
//! v0 fields of a parsed body into the typed model, ignoring every other
//! field. A field is an `Option` exactly where the installed data omits it;
//! the model never invents an engine default.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::io;

use nickel_lang_core::error::report::{ColorOpt, report_as_str};
use nickel_lang_core::error::{Error as NickelError, EvalErrorKind};
use nickel_lang_core::eval::cache::lazy::CBNCache;
use nickel_lang_core::label::ty_path::Elem;
use nickel_lang_core::repl::{Repl as _, ReplImpl};

use serde::{Deserialize, Serialize};
use supercom_bp::{Key, Value};

/// A unit's economy block.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Economy {
    /// Energy cost to build.
    pub build_cost_energy: f64,
    /// Mass cost to build.
    pub build_cost_mass: f64,
    /// Build time; absent in 16 of 568 installed units.
    pub build_time: Option<f64>,
    /// Build rate of a builder.
    pub build_rate: Option<f64>,
    /// Energy produced per second.
    pub production_per_second_energy: Option<f64>,
    /// Mass produced per second.
    pub production_per_second_mass: Option<f64>,
    /// Energy storage.
    pub storage_energy: Option<f64>,
    /// Mass storage.
    pub storage_mass: Option<f64>,
    /// The `BuildableCategory` expressions as stored, one per table entry;
    /// empty when the table is absent (present in 105 of 568 installed units).
    pub buildable_category: Vec<String>,
    /// Energy drained per second while active.
    pub maintenance_consumption_per_second_energy: Option<f64>,
}

/// A unit's physics block.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Physics {
    /// The motion type, such as `RULEUMT_Land`.
    pub motion_type: String,
    /// Maximum speed.
    pub max_speed: Option<f64>,
    /// Turn rate.
    pub turn_rate: Option<f64>,
    /// Maximum acceleration.
    pub max_acceleration: Option<f64>,
    /// Maximum braking.
    pub max_brake: Option<f64>,
    /// Cruise height of an air or sea-level unit.
    pub elevation: Option<f64>,
    /// Minimum speed as a percentage of the maximum.
    pub min_speed_percent: Option<f64>,
    /// Where the unit may be built, such as `RULEUBR_OnMassDeposit`.
    pub build_restriction: Option<String>,
    /// The `BuildOnLayerCaps` keys whose value is true, sorted ascending;
    /// empty when the table is absent.
    pub build_on_layer_caps: Vec<String>,
}

/// A unit's footprint table; the top-level `Footprint` of 402 of 568
/// installed units.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Footprint {
    /// Footprint width in cells.
    pub size_x: Option<f64>,
    /// Footprint depth in cells.
    pub size_z: Option<f64>,
    /// Shallowest water the unit may occupy.
    pub min_water_depth: Option<f64>,
}

/// A unit's defence block.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Defense {
    /// Starting health.
    pub health: f64,
    /// Maximum health.
    pub max_health: f64,
    /// The armour type name.
    pub armor_type: String,
    /// Regeneration per second.
    pub regen_rate: Option<f64>,
}

/// A unit's intel block; present in 395 of 568 installed units.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Intel {
    /// Vision radius; absent in 4 of 395 installed blocks.
    pub vision_radius: Option<f64>,
    /// Radar radius.
    pub radar_radius: Option<f64>,
}

/// One entry of a unit's weapon table.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Weapon {
    /// The weapon's label.
    pub label: Option<String>,
    /// Damage per shot.
    pub damage: f64,
    /// Shots per second; absent in 91 of 494 installed entries.
    pub rate_of_fire: Option<f64>,
    /// Maximum range; absent in 86 of 494 installed entries.
    pub max_radius: Option<f64>,
    /// The projectile blueprint it fires.
    pub projectile_id: Option<String>,
    /// Splash radius.
    pub damage_radius: Option<f64>,
    /// The damage type name, such as `Normal`.
    pub damage_type: String,
    /// Whether the splash hurts friendly units.
    pub damage_friendly: Option<bool>,
    /// Minimum range.
    pub min_radius: Option<f64>,
    /// Muzzle velocity.
    pub muzzle_velocity: Option<f64>,
    /// Projectiles spawned per firing.
    pub projectiles_per_on_fire: Option<f64>,
    /// Shots per salvo.
    pub muzzle_salvo_size: Option<f64>,
    /// Delay between the shots of a salvo.
    pub muzzle_salvo_delay: Option<f64>,
    /// Reload time of a rack salvo.
    pub rack_salvo_reload_time: Option<f64>,
    /// Charge time of a rack salvo.
    pub rack_salvo_charge_time: Option<f64>,
    /// Whether the rack fires after charging.
    pub rack_salvo_fires_after_charge: Option<bool>,
    /// The ballistic arc, such as `RULEUBA_LowArc`.
    pub ballistic_arc: Option<String>,
    /// The target priority expressions as stored; empty when absent.
    pub target_priorities: Vec<String>,
    /// `(layer, caps)` pairs of `FireTargetLayerCapsTable`, sorted by layer.
    pub fire_target_layer_caps: Vec<(String, String)>,
    /// Tracking radius.
    pub tracking_radius: Option<f64>,
    /// Firing tolerance.
    pub firing_tolerance: Option<f64>,
    /// Seconds between target checks.
    pub target_check_interval: Option<f64>,
    /// The weapon category name.
    pub weapon_category: String,
    /// Whether the weapon turns on a turret.
    pub turreted: Option<bool>,
    /// Manual fire; stored as a boolean or as the number 1, which reads as true.
    pub manual_fire: Option<bool>,
    /// The enhancement that must be built before the weapon may fire; `None`
    /// when the entry carries no such field.
    pub enabled_by_enhancement: Option<String>,
}

/// A unit blueprint's v0 fields.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Unit {
    /// The blueprint id, such as the file stem without `_unit`.
    pub id: String,
    /// The category strings.
    pub categories: Vec<String>,
    /// The economy block.
    pub economy: Economy,
    /// The physics block.
    pub physics: Physics,
    /// The defence block.
    pub defense: Defense,
    /// The intel block, when the unit has one.
    pub intel: Option<Intel>,
    /// The weapon table, empty when the unit has none.
    pub weapons: Vec<Weapon>,
    /// The footprint table, when the unit has one.
    pub footprint: Option<Footprint>,
    /// The `General.CommandCaps` keys whose value is true, sorted ascending.
    pub command_caps: Vec<String>,
}

/// A projectile's physics block; present in all 365 installed projectiles.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ProjectilePhysics {
    /// Speed at launch.
    pub initial_speed: Option<f64>,
    /// Maximum speed.
    pub max_speed: Option<f64>,
    /// Acceleration.
    pub acceleration: Option<f64>,
    /// Turn rate.
    pub turn_rate: Option<f64>,
    /// Lifetime in seconds.
    pub lifetime: Option<f64>,
    /// Whether it tracks its target.
    pub track_target: Option<bool>,
    /// Whether gravity acts on it.
    pub use_gravity: Option<bool>,
    /// Whether it leads its target.
    pub lead_target: Option<bool>,
    /// Whether it aligns to its velocity.
    pub velocity_align: Option<bool>,
    /// Whether it is destroyed on touching water.
    pub destroy_on_water: Option<bool>,
}

/// A projectile blueprint's v0 fields.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Projectile {
    /// The blueprint id.
    pub id: String,
    /// The category strings.
    pub categories: Vec<String>,
    /// The physics block.
    pub physics: ProjectilePhysics,
}

/// A prop blueprint's v0 fields.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Prop {
    /// The blueprint id.
    pub id: String,
    /// The category strings.
    pub categories: Vec<String>,
}

/// Which contract [`validate`] applies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// `nickel/unit.ncl`.
    Unit,
    /// `nickel/projectile.ncl`.
    Projectile,
    /// `nickel/prop.ncl`.
    Prop,
}

/// Why a body failed validation or projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// A contract rejected the body.
    Contract {
        /// The path of the offending field.
        path: String,
        /// What the contract said.
        reason: String,
    },
    /// A required field is absent.
    Missing {
        /// The field's name, such as `Economy`.
        field: String,
    },
    /// A field has the wrong type.
    Type {
        /// The field's name or path.
        field: String,
        /// The expected type.
        expected: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract { path, reason } => write!(f, "contract violated at {path}: {reason}"),
            Self::Missing { field } => write!(f, "missing field {field}"),
            Self::Type { field, expected } => write!(f, "field {field} is not {expected}"),
        }
    }
}

impl std::error::Error for Error {}

/// The committed contracts, with the category vocabulary spliced in where
/// `unit.ncl` imports it (the program has no file system to import from).
const CATEGORIES_NCL: &str = include_str!("../nickel/categories.ncl");
const UNIT_NCL: &str = include_str!("../nickel/unit.ncl");
const PROJECTILE_NCL: &str = include_str!("../nickel/projectile.ncl");
const PROP_NCL: &str = include_str!("../nickel/prop.ncl");

/// The top-level keys each contract names, with what it declares of each.
/// The contracts are open records, so a field outside these cannot change a
/// verdict; handing Nickel only these keeps the program small and bounds
/// its depth whatever the body holds.
const UNIT_KEYS: &[(&str, Decl)] = &[
    ("Categories", Decl::Categories),
    ("Economy", Decl::Record(ECONOMY)),
    ("Physics", Decl::Record(PHYSICS)),
    ("Defense", Decl::Record(DEFENSE)),
    ("Intel", Decl::Record(INTEL)),
    ("Weapon", Decl::Records(WEAPON)),
    ("General", Decl::Record(GENERAL)),
    ("Footprint", Decl::Record(FOOTPRINT)),
];
const PROJECTILE_KEYS: &[(&str, Decl)] = &[
    ("Categories", Decl::Categories),
    ("Physics", Decl::Record(PROJECTILE_PHYSICS)),
];
const CATEGORY_KEYS: &[(&str, Decl)] = &[("Categories", Decl::Categories)];

/// The Nickel expression of the contract for `kind`.
fn contract_source(kind: Kind) -> String {
    let (text, name) = match kind {
        Kind::Unit => (UNIT_NCL, "Unit"),
        Kind::Projectile => (PROJECTILE_NCL, "Projectile"),
        Kind::Prop => (PROP_NCL, "Prop"),
    };
    let spliced = text.replace("import \"categories.ncl\"", &format!("({CATEGORIES_NCL})"));
    format!("let contracts = ({spliced}) in contracts.{name}")
}

/// Writes `text` as a Nickel string literal.
fn nickel_string(out: &mut String, text: &str) {
    out.push('"');
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '%' if chars.peek() == Some(&'{') => out.push_str("\\%"),
            c if c.is_ascii_control() => {
                let _ = write!(out, "\\x{:02x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// What the contracts declare of a value: the renderer writes only this and
/// never an undeclared field or subtree, so no depth of a crafted body can
/// reach the evaluator.
#[derive(Clone, Copy)]
enum Decl {
    /// A scalar leaf; a table or call is written as `{}`, which every leaf
    /// contract (`Number`, `String`) rejects as before.
    Leaf,
    /// The `Categories` list, in full for the vocabulary check; its strings
    /// are enum tags when the flag of [`nickel_decl`] is set.
    Categories,
    /// A record of the named leaf fields.
    Record(&'static [&'static str]),
    /// An array of such records.
    Records(&'static [&'static str]),
    /// A positional list of leaves.
    List,
    /// A table of named leaves, whatever the names.
    Map,
}

/// What the contracts declare of the value of a record field: the lists and
/// maps among the leaf fields.
fn field_decl(name: &str) -> Decl {
    match name {
        "BuildableCategory" | "TargetPriorities" => Decl::List,
        "CommandCaps" | "FireTargetLayerCapsTable" | "BuildOnLayerCaps" => Decl::Map,
        _ => Decl::Leaf,
    }
}

const ECONOMY: &[&str] = &[
    "BuildCostEnergy",
    "BuildCostMass",
    "BuildTime",
    "BuildRate",
    "ProductionPerSecondEnergy",
    "ProductionPerSecondMass",
    "StorageEnergy",
    "StorageMass",
    "BuildableCategory",
    "MaintenanceConsumptionPerSecondEnergy",
];
const PHYSICS: &[&str] = &[
    "MotionType",
    "MaxSpeed",
    "TurnRate",
    "MaxAcceleration",
    "MaxBrake",
    "Elevation",
    "MinSpeedPercent",
    "BuildRestriction",
    "BuildOnLayerCaps",
];
const DEFENSE: &[&str] = &["Health", "MaxHealth", "ArmorType", "RegenRate"];
const INTEL: &[&str] = &["VisionRadius", "RadarRadius"];
const WEAPON: &[&str] = &[
    "Label",
    "Damage",
    "RateOfFire",
    "MaxRadius",
    "ProjectileId",
    "DamageRadius",
    "DamageType",
    "DamageFriendly",
    "MinRadius",
    "MuzzleVelocity",
    "ProjectilesPerOnFire",
    "MuzzleSalvoSize",
    "MuzzleSalvoDelay",
    "RackSalvoReloadTime",
    "RackSalvoChargeTime",
    "RackSalvoFiresAfterCharge",
    "BallisticArc",
    "TargetPriorities",
    "FireTargetLayerCapsTable",
    "TrackingRadius",
    "FiringTolerance",
    "TargetCheckInterval",
    "WeaponCategory",
    "Turreted",
    "ManualFire",
    "EnabledByEnhancement",
];
const GENERAL: &[&str] = &["CommandCaps"];
const FOOTPRINT: &[&str] = &["SizeX", "SizeZ", "MinWaterDepth"];
const PROJECTILE_PHYSICS: &[&str] = &[
    "InitialSpeed",
    "MaxSpeed",
    "Acceleration",
    "TurnRate",
    "Lifetime",
    "TrackTarget",
    "UseGravity",
    "LeadTarget",
    "VelocityAlign",
    "DestroyOnWater",
];

/// Writes `value` as a Nickel term, limited to what `decl` declares. A table
/// whose keys are all positional is an array, any other table a record (a
/// positional key in a mixed table is undeclared and dropped); a `<LOC>`
/// string is `{ loc, text }`. With `tags`, the strings of `Categories` are
/// written as enum tags, the form the category contract takes.
fn nickel_decl(out: &mut String, value: &Value, decl: Decl, tags: bool) {
    match value {
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) if n.is_finite() => {
            let _ = write!(out, "({n})");
        }
        Value::Nil | Value::Number(_) => out.push_str("null"),
        Value::Str(s) if tags && matches!(decl, Decl::Leaf) => {
            out.push('\'');
            nickel_string(out, s);
        }
        Value::Str(s) => nickel_string(out, s),
        Value::Loc { id, text } => {
            out.push_str("{ loc = ");
            nickel_string(out, id);
            out.push_str(", text = ");
            nickel_string(out, text);
            out.push_str(" }");
        }
        Value::Call { .. } => out.push_str("{}"),
        Value::Table(entries) => {
            let positional = entries.iter().all(|(k, _)| matches!(k, Key::Index(_)));
            match (decl, positional) {
                (Decl::Categories | Decl::List, true) => {
                    out.push('[');
                    for (_, v) in entries {
                        nickel_decl(out, v, Decl::Leaf, tags);
                        out.push_str(", ");
                    }
                    out.push(']');
                }
                (Decl::Records(fields), true) => {
                    out.push('[');
                    for (_, v) in entries {
                        nickel_decl(out, v, Decl::Record(fields), false);
                        out.push_str(", ");
                    }
                    out.push(']');
                }
                // A leaf, or a record where a list is declared: rejected by type.
                (Decl::Leaf, _) | (Decl::Categories | Decl::Records(_) | Decl::List, false) => {
                    out.push_str("{}");
                }
                // The named leaves of a map; a positional entry is undeclared.
                (Decl::Map, _) => {
                    out.push('{');
                    let mut seen = BTreeSet::new();
                    for (key, v) in entries {
                        let Key::Name(name) = key else { continue };
                        if matches!(v, Value::Nil) || !seen.insert(name.as_str()) {
                            continue;
                        }
                        nickel_string(out, name);
                        out.push_str(" = ");
                        nickel_decl(out, v, Decl::Leaf, false);
                        out.push_str(", ");
                    }
                    out.push('}');
                }
                // A record of its declared named fields, whether the table is
                // empty, positional, or mixed (a positional entry is undeclared);
                // a declared field holding nil is absent, as the Rust checks
                // read it.
                (Decl::Record(fields), _) => {
                    out.push('{');
                    let mut seen = BTreeSet::new();
                    for (key, v) in entries {
                        let Key::Name(name) = key else { continue };
                        if matches!(v, Value::Nil)
                            || !fields.contains(&name.as_str())
                            || !seen.insert(name.as_str())
                        {
                            continue;
                        }
                        nickel_string(out, name);
                        out.push_str(" = ");
                        nickel_decl(out, v, field_decl(name), false);
                        out.push_str(", ");
                    }
                    out.push('}');
                }
            }
        }
    }
}

/// The record of the contract-declared parts of `body`, as Nickel text; with
/// `tags` the category strings are enum tags.
fn nickel_body(body: &Value, keys: &[(&str, Decl)], tags: bool) -> String {
    let mut out = String::from("{");
    for (key, decl) in keys {
        if let Some(value) = body.get(key).filter(|v| !matches!(v, Value::Nil)) {
            nickel_string(&mut out, key);
            out.push_str(" = ");
            nickel_decl(&mut out, value, *decl, tags);
            out.push_str(", ");
        }
    }
    out.push('}');
    out
}

fn contract_error(path: &str, reason: impl Into<String>) -> Error {
    Error::Contract {
        path: path.to_owned(),
        reason: reason.into(),
    }
}

/// Renders a label path such as `Categories[]`.
fn render_path(path: &[Elem]) -> String {
    let mut out = String::new();
    for elem in path {
        match elem {
            Elem::Field(id) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(id.label());
            }
            Elem::Array => out.push_str("[]"),
            Elem::Dict => out.push_str(".{}"),
            Elem::Domain | Elem::Codomain => {}
        }
    }
    out
}

thread_local! {
    /// One Nickel session per thread, with the standard library loaded and
    /// the three contracts bound once; building a program per body costs
    /// about 4 ms of standard-library setup, which would not fit the load
    /// budget across 1,269 bodies.
    static SESSION: RefCell<Option<ReplImpl<CBNCache>>> = const { RefCell::new(None) };
}

/// The name a kind's contract is bound to in the session.
fn contract_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Unit => "contract_unit",
        Kind::Projectile => "contract_projectile",
        Kind::Prop => "contract_prop",
    }
}

/// A session with the contracts bound.
fn new_session() -> Result<ReplImpl<CBNCache>, NickelError> {
    let mut repl = ReplImpl::new(io::sink());
    repl.load_stdlib()?;
    for kind in [Kind::Unit, Kind::Projectile, Kind::Prop] {
        repl.eval_full(&format!(
            "let {} = {}",
            contract_name(kind),
            contract_source(kind)
        ))?;
    }
    Ok(repl)
}

/// The first line of Nickel's report of `error`.
fn first_line(repl: &mut ReplImpl<CBNCache>, error: NickelError) -> String {
    let mut files = repl.cache_mut().sources.files().clone();
    let report = report_as_str(&mut files, error, ColorOpt::Never);
    report
        .lines()
        .next()
        .unwrap_or("contract violated")
        .to_owned()
}

/// Evaluates `body | contract` in-process and maps a blame to
/// [`Error::Contract`]. The Rust checks reject every body the contract
/// rejects except a category outside the vocabulary (the agreement tests
/// under `tests/` pin this), so the only constraint left for Nickel to
/// decide is the vocabulary, and a blame whose path does not name a field
/// is attributed to `Categories`.
fn nickel_check(kind: Kind, body: &Value) -> Result<(), Error> {
    let keys = match kind {
        Kind::Unit => UNIT_KEYS,
        Kind::Projectile => PROJECTILE_KEYS,
        Kind::Prop => CATEGORY_KEYS,
    };
    let source = format!(
        "({}) | {}",
        nickel_body(body, keys, kind == Kind::Unit),
        contract_name(kind)
    );
    SESSION.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let session = new_session()
                .map_err(|e| contract_error("", format!("the contracts did not load: {e:?}")))?;
            *slot = Some(session);
        }
        let Some(repl) = slot.as_mut() else {
            return Err(contract_error("", "no Nickel session"));
        };
        match repl.eval_full(&source) {
            Ok(_) => Ok(()),
            Err(error) => {
                let mut path = match &error {
                    NickelError::EvalError(data) => match &data.error {
                        EvalErrorKind::BlameError { label, .. } => {
                            render_path(&label.path.iter().copied().collect::<Vec<_>>())
                        }
                        _ => String::new(),
                    },
                    _ => String::new(),
                };
                if !path.starts_with(|c: char| c.is_alphabetic()) {
                    path.insert_str(0, "Categories");
                }
                Err(contract_error(&path, first_line(repl, error)))
            }
        }
    })
}

/// A table entry required to be present (and not `nil`).
fn required<'a>(table: &'a Value, key: &str, path: &str) -> Result<&'a Value, Error> {
    table
        .get(key)
        .filter(|v| !matches!(v, Value::Nil))
        .ok_or_else(|| Error::Missing {
            field: path.to_owned(),
        })
}

fn type_error(path: &str, expected: &str) -> Error {
    Error::Type {
        field: path.to_owned(),
        expected: expected.to_owned(),
    }
}

/// A finite number; Lua reads `1e999` as infinity, which the model refuses
/// as it refuses any value the contract's `Number` would reject.
fn number_at(value: &Value, path: &str) -> Result<f64, Error> {
    value
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| type_error(path, "Number"))
}

fn string_at(value: &Value, path: &str) -> Result<String, Error> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| type_error(path, "String"))
}

fn record_at<'a>(value: &'a Value, path: &str) -> Result<&'a Value, Error> {
    match value {
        Value::Table(_) => Ok(value),
        _ => Err(type_error(path, "Record")),
    }
}

fn path_of(parent: &str, key: &str) -> String {
    format!("{parent}.{key}")
}

fn req_number(table: &Value, key: &str, parent: &str) -> Result<f64, Error> {
    let path = path_of(parent, key);
    number_at(required(table, key, &path)?, &path)
}

fn req_string(table: &Value, key: &str, parent: &str) -> Result<String, Error> {
    let path = path_of(parent, key);
    string_at(required(table, key, &path)?, &path)
}

fn opt_number(table: &Value, key: &str, parent: &str) -> Result<Option<f64>, Error> {
    match table.get(key) {
        None | Some(Value::Nil) => Ok(None),
        Some(v) => number_at(v, &path_of(parent, key)).map(Some),
    }
}

fn opt_string(table: &Value, key: &str, parent: &str) -> Result<Option<String>, Error> {
    match table.get(key) {
        None | Some(Value::Nil) => Ok(None),
        Some(v) => string_at(v, &path_of(parent, key)).map(Some),
    }
}

fn bool_at(value: &Value, path: &str) -> Result<bool, Error> {
    match value {
        Value::Bool(b) => Ok(*b),
        _ => Err(type_error(path, "Boolean")),
    }
}

fn opt_bool(table: &Value, key: &str, parent: &str) -> Result<Option<bool>, Error> {
    match table.get(key) {
        None | Some(Value::Nil) => Ok(None),
        Some(v) => bool_at(v, &path_of(parent, key)).map(Some),
    }
}

/// `ManualFire` is a boolean on 13 installed entries and the number 1 on 9;
/// the number 1 reads as true and any other value is a type error, the
/// predicate `unit.ncl` states for the same field.
fn opt_manual_fire(table: &Value, key: &str, parent: &str) -> Result<Option<bool>, Error> {
    match table.get(key) {
        None | Some(Value::Nil) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(Value::Number(n)) if n.to_bits() == 1.0_f64.to_bits() => Ok(Some(true)),
        Some(_) => Err(type_error(&path_of(parent, key), "Boolean")),
    }
}

/// An absent list is empty; a present one is an array of strings.
fn opt_string_list(table: &Value, key: &str, parent: &str) -> Result<Vec<String>, Error> {
    let path = path_of(parent, key);
    match table.get(key) {
        None | Some(Value::Nil) => Ok(Vec::new()),
        Some(v) => array_at(v, &path)?
            .entries()
            .iter()
            .enumerate()
            .map(|(i, (_, item))| string_at(item, &format!("{path}[{i}]")))
            .collect(),
    }
}

/// The named, non-nil entries of a dictionary table, the first of each
/// name, sorted by name; the nil and positional entries are undeclared, as
/// the bridge treats them.
fn named_entries(table: &Value) -> BTreeMap<&str, &Value> {
    let mut map = BTreeMap::new();
    for (key, value) in table.entries() {
        if let Key::Name(name) = key
            && !matches!(value, Value::Nil)
        {
            map.entry(name.as_str()).or_insert(value);
        }
    }
    map
}

/// As [`categories_of`], but an absent table is an empty list (projectiles
/// and props omit it in the installed data).
fn optional_categories_of(body: &Value) -> Result<Vec<String>, Error> {
    match body.get("Categories") {
        None | Some(Value::Nil) => Ok(Vec::new()),
        Some(_) => categories_of(body),
    }
}

/// A table declared as an array must carry only positional entries, the
/// shape the contract's `Array` accepts.
fn array_at<'a>(value: &'a Value, path: &str) -> Result<&'a Value, Error> {
    match value {
        Value::Table(entries) if entries.iter().all(|(k, _)| matches!(k, Key::Index(_))) => {
            Ok(value)
        }
        _ => Err(type_error(path, "Array")),
    }
}

fn categories_of(body: &Value) -> Result<Vec<String>, Error> {
    let list = array_at(required(body, "Categories", "Categories")?, "Categories")?;
    list.entries()
        .iter()
        .enumerate()
        .map(|(i, (_, v))| string_at(v, &format!("Categories[{i}]")))
        .collect()
}

fn economy_of(body: &Value) -> Result<Economy, Error> {
    let t = record_at(required(body, "Economy", "Economy")?, "Economy")?;
    Ok(Economy {
        build_cost_energy: req_number(t, "BuildCostEnergy", "Economy")?,
        build_cost_mass: req_number(t, "BuildCostMass", "Economy")?,
        build_time: opt_number(t, "BuildTime", "Economy")?,
        build_rate: opt_number(t, "BuildRate", "Economy")?,
        production_per_second_energy: opt_number(t, "ProductionPerSecondEnergy", "Economy")?,
        production_per_second_mass: opt_number(t, "ProductionPerSecondMass", "Economy")?,
        storage_energy: opt_number(t, "StorageEnergy", "Economy")?,
        storage_mass: opt_number(t, "StorageMass", "Economy")?,
        buildable_category: opt_string_list(t, "BuildableCategory", "Economy")?,
        maintenance_consumption_per_second_energy: opt_number(
            t,
            "MaintenanceConsumptionPerSecondEnergy",
            "Economy",
        )?,
    })
}

fn physics_of(body: &Value) -> Result<Physics, Error> {
    let t = record_at(required(body, "Physics", "Physics")?, "Physics")?;
    Ok(Physics {
        motion_type: req_string(t, "MotionType", "Physics")?,
        max_speed: opt_number(t, "MaxSpeed", "Physics")?,
        turn_rate: opt_number(t, "TurnRate", "Physics")?,
        max_acceleration: opt_number(t, "MaxAcceleration", "Physics")?,
        max_brake: opt_number(t, "MaxBrake", "Physics")?,
        elevation: opt_number(t, "Elevation", "Physics")?,
        min_speed_percent: opt_number(t, "MinSpeedPercent", "Physics")?,
        build_restriction: opt_string(t, "BuildRestriction", "Physics")?,
        build_on_layer_caps: true_keys(t, "BuildOnLayerCaps", "Physics")?,
    })
}

fn defense_of(body: &Value) -> Result<Defense, Error> {
    let t = record_at(required(body, "Defense", "Defense")?, "Defense")?;
    Ok(Defense {
        health: req_number(t, "Health", "Defense")?,
        max_health: req_number(t, "MaxHealth", "Defense")?,
        armor_type: req_string(t, "ArmorType", "Defense")?,
        regen_rate: opt_number(t, "RegenRate", "Defense")?,
    })
}

fn intel_of(body: &Value) -> Result<Option<Intel>, Error> {
    match body.get("Intel") {
        None | Some(Value::Nil) => Ok(None),
        Some(v) => {
            let t = record_at(v, "Intel")?;
            Ok(Some(Intel {
                vision_radius: opt_number(t, "VisionRadius", "Intel")?,
                radar_radius: opt_number(t, "RadarRadius", "Intel")?,
            }))
        }
    }
}

fn weapons_of(body: &Value) -> Result<Vec<Weapon>, Error> {
    let list = match body.get("Weapon") {
        None | Some(Value::Nil) => return Ok(Vec::new()),
        Some(v) => array_at(v, "Weapon")?,
    };
    list.entries()
        .iter()
        .enumerate()
        .map(|(i, (_, entry))| {
            let parent = format!("Weapon[{i}]");
            let t = record_at(entry, &parent)?;
            Ok(Weapon {
                label: opt_string(t, "Label", &parent)?,
                damage: req_number(t, "Damage", &parent)?,
                rate_of_fire: opt_number(t, "RateOfFire", &parent)?,
                max_radius: opt_number(t, "MaxRadius", &parent)?,
                projectile_id: opt_string(t, "ProjectileId", &parent)?,
                damage_radius: opt_number(t, "DamageRadius", &parent)?,
                damage_type: req_string(t, "DamageType", &parent)?,
                damage_friendly: opt_bool(t, "DamageFriendly", &parent)?,
                min_radius: opt_number(t, "MinRadius", &parent)?,
                muzzle_velocity: opt_number(t, "MuzzleVelocity", &parent)?,
                projectiles_per_on_fire: opt_number(t, "ProjectilesPerOnFire", &parent)?,
                muzzle_salvo_size: opt_number(t, "MuzzleSalvoSize", &parent)?,
                muzzle_salvo_delay: opt_number(t, "MuzzleSalvoDelay", &parent)?,
                rack_salvo_reload_time: opt_number(t, "RackSalvoReloadTime", &parent)?,
                rack_salvo_charge_time: opt_number(t, "RackSalvoChargeTime", &parent)?,
                rack_salvo_fires_after_charge: opt_bool(t, "RackSalvoFiresAfterCharge", &parent)?,
                ballistic_arc: opt_string(t, "BallisticArc", &parent)?,
                target_priorities: opt_string_list(t, "TargetPriorities", &parent)?,
                fire_target_layer_caps: fire_target_layer_caps_of(t, &parent)?,
                tracking_radius: opt_number(t, "TrackingRadius", &parent)?,
                firing_tolerance: opt_number(t, "FiringTolerance", &parent)?,
                target_check_interval: opt_number(t, "TargetCheckInterval", &parent)?,
                weapon_category: req_string(t, "WeaponCategory", &parent)?,
                turreted: opt_bool(t, "Turreted", &parent)?,
                manual_fire: opt_manual_fire(t, "ManualFire", &parent)?,
                enabled_by_enhancement: opt_string(t, "EnabledByEnhancement", &parent)?,
            })
        })
        .collect()
}

/// The `(layer, caps)` pairs of `FireTargetLayerCapsTable`, sorted by layer.
fn fire_target_layer_caps_of(weapon: &Value, parent: &str) -> Result<Vec<(String, String)>, Error> {
    let path = path_of(parent, "FireTargetLayerCapsTable");
    match weapon.get("FireTargetLayerCapsTable") {
        None | Some(Value::Nil) => Ok(Vec::new()),
        Some(v) => named_entries(record_at(v, &path)?)
            .into_iter()
            .map(|(layer, caps)| Ok((layer.to_owned(), string_at(caps, &path_of(&path, layer))?)))
            .collect(),
    }
}

fn footprint_of(body: &Value) -> Result<Option<Footprint>, Error> {
    match body.get("Footprint") {
        None | Some(Value::Nil) => Ok(None),
        Some(v) => {
            let t = record_at(v, "Footprint")?;
            Ok(Some(Footprint {
                size_x: opt_number(t, "SizeX", "Footprint")?,
                size_z: opt_number(t, "SizeZ", "Footprint")?,
                min_water_depth: opt_number(t, "MinWaterDepth", "Footprint")?,
            }))
        }
    }
}

/// The keys of the Boolean table `caps` (at `path`) whose value is true,
/// sorted ascending.
fn true_keys_of(caps: &Value, path: &str) -> Result<Vec<String>, Error> {
    let mut keys = Vec::new();
    for (key, value) in named_entries(caps) {
        if bool_at(value, &path_of(path, key))? {
            keys.push(key.to_owned());
        }
    }
    Ok(keys)
}

/// As [`true_keys_of`] for the table `key` of `table`; empty when absent.
fn true_keys(table: &Value, key: &str, parent: &str) -> Result<Vec<String>, Error> {
    let path = path_of(parent, key);
    match table.get(key) {
        None | Some(Value::Nil) => Ok(Vec::new()),
        Some(v) => true_keys_of(record_at(v, &path)?, &path),
    }
}

/// The `General.CommandCaps` keys whose value is true, sorted ascending.
fn command_caps_of(body: &Value) -> Result<Vec<String>, Error> {
    let general = record_at(required(body, "General", "General")?, "General")?;
    let path = "General.CommandCaps";
    true_keys_of(
        record_at(required(general, "CommandCaps", path)?, path)?,
        path,
    )
}

fn projectile_physics_of(body: &Value) -> Result<ProjectilePhysics, Error> {
    let t = record_at(required(body, "Physics", "Physics")?, "Physics")?;
    Ok(ProjectilePhysics {
        initial_speed: opt_number(t, "InitialSpeed", "Physics")?,
        max_speed: opt_number(t, "MaxSpeed", "Physics")?,
        acceleration: opt_number(t, "Acceleration", "Physics")?,
        turn_rate: opt_number(t, "TurnRate", "Physics")?,
        lifetime: opt_number(t, "Lifetime", "Physics")?,
        track_target: opt_bool(t, "TrackTarget", "Physics")?,
        use_gravity: opt_bool(t, "UseGravity", "Physics")?,
        lead_target: opt_bool(t, "LeadTarget", "Physics")?,
        velocity_align: opt_bool(t, "VelocityAlign", "Physics")?,
        destroy_on_water: opt_bool(t, "DestroyOnWater", "Physics")?,
    })
}

/// Applies the contract for `kind` to `body` through `nickel-lang-core`.
///
/// The presence and type of the v0 fields are checked in Rust first, in the
/// order the contracts name them, so a missing block is
/// [`Error::Missing`] and a wrong type [`Error::Type`]; only then does
/// Nickel apply the contract (which is what rejects a category outside the
/// vocabulary).
///
/// # Errors
///
/// [`Error::Missing`], [`Error::Type`], or [`Error::Contract`] naming the
/// first violation.
pub fn validate(kind: Kind, body: &Value) -> Result<(), Error> {
    match kind {
        Kind::Unit => {
            unit_from_value("", body)?;
        }
        Kind::Projectile => {
            projectile_from_value("", body)?;
        }
        Kind::Prop => {
            optional_categories_of(body)?;
        }
    }
    nickel_check(kind, body)
}

/// Projects the v0 fields of a unit blueprint body, ignoring the rest.
///
/// # Errors
///
/// [`Error::Missing`] or [`Error::Type`] on the first v0 field that is
/// absent or mistyped.
pub fn unit_from_value(id: &str, body: &Value) -> Result<Unit, Error> {
    Ok(Unit {
        id: id.to_owned(),
        categories: categories_of(body)?,
        economy: economy_of(body)?,
        physics: physics_of(body)?,
        defense: defense_of(body)?,
        intel: intel_of(body)?,
        weapons: weapons_of(body)?,
        footprint: footprint_of(body)?,
        command_caps: command_caps_of(body)?,
    })
}

/// Projects the v0 fields of a projectile blueprint body.
///
/// # Errors
///
/// As [`unit_from_value`].
pub fn projectile_from_value(id: &str, body: &Value) -> Result<Projectile, Error> {
    Ok(Projectile {
        id: id.to_owned(),
        categories: optional_categories_of(body)?,
        physics: projectile_physics_of(body)?,
    })
}

/// Projects the v0 fields of a prop blueprint body.
///
/// # Errors
///
/// As [`unit_from_value`].
pub fn prop_from_value(id: &str, body: &Value) -> Result<Prop, Error> {
    Ok(Prop {
        id: id.to_owned(),
        categories: optional_categories_of(body)?,
    })
}

#[cfg(test)]
mod declared_fields {
    //! The field names the contracts declare, read from their text, equal
    //! the renderer's lists, so the two cannot drift apart silently.

    use std::collections::BTreeSet;

    use super::{
        DEFENSE, ECONOMY, FOOTPRINT, GENERAL, INTEL, PHYSICS, PROJECTILE_KEYS, PROJECTILE_NCL,
        PROJECTILE_PHYSICS, UNIT_KEYS, UNIT_NCL, WEAPON,
    };

    /// The identifiers `source` declares, keyed by top-level block.
    fn declared(source: &str) -> Vec<(String, BTreeSet<String>)> {
        let mut blocks: Vec<(String, BTreeSet<String>)> = Vec::new();
        for line in source.lines() {
            let indent = line.len() - line.trim_start().len();
            let word: String = line
                .trim_start()
                .chars()
                .take_while(char::is_ascii_alphabetic)
                .collect();
            if word.is_empty() {
                continue;
            }
            match (indent, blocks.last_mut()) {
                (4, _) => blocks.push((word, BTreeSet::new())),
                (8, Some((_, fields))) => {
                    fields.insert(word);
                }
                _ => {}
            }
        }
        blocks
    }

    fn set(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|f| (*f).to_owned()).collect()
    }

    #[test]
    fn the_unit_contract_declares_the_rendered_keys() {
        let blocks = declared(UNIT_NCL);
        let keys: BTreeSet<String> = blocks.iter().map(|(k, _)| k.clone()).collect();
        let rendered: BTreeSet<String> = UNIT_KEYS.iter().map(|(k, _)| (*k).to_owned()).collect();
        assert_eq!(keys, rendered);
        for (block, list) in [
            ("Economy", ECONOMY),
            ("Physics", PHYSICS),
            ("Defense", DEFENSE),
            ("Intel", INTEL),
            ("Weapon", WEAPON),
            ("General", GENERAL),
            ("Footprint", FOOTPRINT),
        ] {
            let fields = blocks
                .iter()
                .find(|(k, _)| k == block)
                .map(|(_, f)| f.clone())
                .unwrap_or_default();
            assert_eq!(fields, set(list), "{block}");
        }
    }

    #[test]
    fn the_projectile_contract_declares_the_rendered_keys() {
        let blocks = declared(PROJECTILE_NCL);
        let keys: BTreeSet<String> = blocks.iter().map(|(k, _)| k.clone()).collect();
        let rendered: BTreeSet<String> = PROJECTILE_KEYS
            .iter()
            .map(|(k, _)| (*k).to_owned())
            .collect();
        assert_eq!(keys, rendered);
        let physics = blocks
            .iter()
            .find(|(k, _)| k == "Physics")
            .map(|(_, f)| f.clone())
            .unwrap_or_default();
        assert_eq!(physics, set(PROJECTILE_PHYSICS));
    }
}

#[cfg(test)]
mod refusals {
    //! The contract's refuse side, exercised directly: each body below is one
    //! the projection also refuses, so [`validate`] never reaches Nickel with
    //! it, and only a call to [`nickel_check`] shows the contract itself
    //! holds the line.

    use supercom_bp::{Key, Value};

    use super::{Kind, nickel_check};

    fn table(entries: Vec<(&str, Value)>) -> Value {
        Value::Table(
            entries
                .into_iter()
                .map(|(k, v)| (Key::Name(k.to_owned()), v))
                .collect(),
        )
    }

    fn s(text: &str) -> Value {
        Value::Str(text.to_owned())
    }

    fn num(n: f64) -> Value {
        Value::Number(n)
    }

    fn flag(b: bool) -> Value {
        Value::Bool(b)
    }

    fn unit_with_weapon_field(value: Value) -> Value {
        unit_with_weapon("ManualFire", value)
    }

    fn unit_with_weapon(field: &str, value: Value) -> Value {
        table(vec![
            ("Categories", Value::Table(vec![(Key::Index(1), s("LAND"))])),
            (
                "Economy",
                table(vec![
                    ("BuildCostEnergy", num(1.0)),
                    ("BuildCostMass", num(1.0)),
                ]),
            ),
            ("Physics", table(vec![("MotionType", s("RULEUMT_Land"))])),
            (
                "Defense",
                table(vec![
                    ("Health", num(1.0)),
                    ("MaxHealth", num(1.0)),
                    ("ArmorType", s("Normal")),
                ]),
            ),
            ("General", table(vec![("CommandCaps", table(vec![]))])),
            (
                "Weapon",
                Value::Table(vec![(
                    Key::Index(1),
                    table(vec![
                        ("Damage", num(1.0)),
                        ("DamageType", s("Normal")),
                        ("WeaponCategory", s("Direct Fire")),
                        (field, value),
                    ]),
                )]),
            ),
        ])
    }

    fn unit_with_general(caps: Value) -> Value {
        table(vec![
            ("Categories", Value::Table(vec![(Key::Index(1), s("LAND"))])),
            (
                "Economy",
                table(vec![
                    ("BuildCostEnergy", num(1.0)),
                    ("BuildCostMass", num(1.0)),
                ]),
            ),
            ("Physics", table(vec![("MotionType", s("RULEUMT_Land"))])),
            (
                "Defense",
                table(vec![
                    ("Health", num(1.0)),
                    ("MaxHealth", num(1.0)),
                    ("ArmorType", s("Normal")),
                ]),
            ),
            ("General", table(vec![("CommandCaps", caps)])),
        ])
    }

    fn projectile_with_physics(entries: Vec<(&str, Value)>) -> Value {
        table(vec![("Physics", table(entries))])
    }

    #[test]
    fn the_contract_refuses_a_manual_fire_of_two() {
        assert!(nickel_check(Kind::Unit, &unit_with_weapon_field(num(2.0))).is_err());
    }

    #[test]
    fn the_contract_refuses_a_manual_fire_string() {
        assert!(nickel_check(Kind::Unit, &unit_with_weapon_field(s("yes"))).is_err());
    }

    #[test]
    fn the_contract_accepts_the_three_manual_fire_values() {
        for v in [flag(true), flag(false), num(1.0)] {
            assert_eq!(nickel_check(Kind::Unit, &unit_with_weapon_field(v)), Ok(()));
        }
    }

    #[test]
    fn the_contract_refuses_a_string_command_cap() {
        let caps = table(vec![("RULEUCC_Move", s("yes"))]);
        assert!(nickel_check(Kind::Unit, &unit_with_general(caps)).is_err());
    }

    #[test]
    fn the_contract_refuses_a_numeric_fire_target_layer_cap() {
        let table_value = table(vec![("Land", num(3.0))]);
        let body = unit_with_weapon("FireTargetLayerCapsTable", table_value);
        assert!(nickel_check(Kind::Unit, &body).is_err());
    }

    #[test]
    fn the_contract_refuses_a_numeric_track_target() {
        let body = projectile_with_physics(vec![("TrackTarget", num(1.0))]);
        assert!(nickel_check(Kind::Projectile, &body).is_err());
    }

    #[test]
    fn the_projectile_contract_accepts_every_declared_field() {
        let body = table(vec![
            (
                "Categories",
                Value::Table(vec![(Key::Index(1), s("PROJECTILE"))]),
            ),
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
        assert_eq!(nickel_check(Kind::Projectile, &body), Ok(()));
    }

    #[test]
    fn the_contract_refuses_a_non_boolean_layer_cap() {
        let physics = table(vec![
            ("MotionType", s("RULEUMT_Land")),
            ("BuildOnLayerCaps", table(vec![("LAYER_Land", s("yes"))])),
        ]);
        let mut body = unit_with_general(table(vec![]));
        if let Value::Table(entries) = &mut body {
            entries.retain(|(k, _)| *k != Key::Name("Physics".to_owned()));
            entries.push((Key::Name("Physics".to_owned()), physics));
        }
        assert!(nickel_check(Kind::Unit, &body).is_err());
    }
}
