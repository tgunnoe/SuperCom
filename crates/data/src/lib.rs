//! The typed v0 model of unit, projectile, and prop blueprints, and the
//! Nickel contracts that validate them.
//!
//! The contracts live in `nickel/`; [`validate`] applies one in-process
//! through `nickel-lang-core`, and the `*_from_value` functions project the
//! v0 fields of a parsed body into the typed model, ignoring every other
//! field. A field is an `Option` exactly where the installed data omits it;
//! the model never invents an engine default.

use std::cell::RefCell;
use std::collections::BTreeSet;
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
}

/// A projectile blueprint's v0 fields.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Projectile {
    /// The blueprint id.
    pub id: String,
    /// The category strings.
    pub categories: Vec<String>,
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

/// The top-level keys each contract names. The contracts are open records,
/// so a key outside this list cannot change a verdict; handing Nickel only
/// these keeps the program small.
const UNIT_KEYS: &[&str] = &[
    "Categories",
    "Economy",
    "Physics",
    "Defense",
    "Intel",
    "Weapon",
];
const CATEGORY_KEYS: &[&str] = &["Categories"];

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

/// Writes `value` as a Nickel term. A table whose keys are all positional
/// is an array, any other table a record (a positional key in a mixed table
/// becomes a field named `[i]`); a call is `{ call, args }`, a `<LOC>` string
/// `{ loc, text }`. With `tags`, the strings of the array are written as
/// enum tags, the form the category contract takes.
fn nickel_term(out: &mut String, value: &Value, tags: bool) {
    match value {
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) if n.is_finite() => {
            let _ = write!(out, "({n})");
        }
        Value::Nil | Value::Number(_) => out.push_str("null"),
        Value::Str(s) if tags => {
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
        Value::Call { name, args } => {
            out.push_str("{ call = ");
            nickel_string(out, name);
            out.push_str(", args = [");
            for arg in args {
                nickel_term(out, arg, false);
                out.push_str(", ");
            }
            out.push_str("] }");
        }
        Value::Table(entries) => {
            if entries.iter().all(|(k, _)| matches!(k, Key::Index(_))) {
                out.push('[');
                for (_, v) in entries {
                    nickel_term(out, v, tags);
                    out.push_str(", ");
                }
                out.push(']');
            } else {
                out.push('{');
                let mut seen = BTreeSet::new();
                for (key, v) in entries {
                    let name = match key {
                        Key::Name(n) => n.clone(),
                        Key::Index(i) => format!("[{i}]"),
                    };
                    if seen.contains(&name) {
                        continue;
                    }
                    nickel_string(out, &name);
                    out.push_str(" = ");
                    nickel_term(out, v, false);
                    out.push_str(", ");
                    seen.insert(name);
                }
                out.push('}');
            }
        }
    }
}

/// The record of the contract-named keys of `body`, as Nickel text; with
/// `tags` the category strings are enum tags.
fn nickel_body(body: &Value, keys: &[&str], tags: bool) -> String {
    let mut out = String::from("{");
    for key in keys {
        if let Some(value) = body.get(key).filter(|v| !matches!(v, Value::Nil)) {
            nickel_string(&mut out, key);
            out.push_str(" = ");
            nickel_term(&mut out, value, tags && *key == "Categories");
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
/// [`Error::Contract`]. After the Rust checks, the only constraint left for
/// Nickel to decide is the category vocabulary, so a blame whose path does
/// not name a field is attributed to `Categories`.
fn nickel_check(kind: Kind, body: &Value) -> Result<(), Error> {
    let keys = if kind == Kind::Unit {
        UNIT_KEYS
    } else {
        CATEGORY_KEYS
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

fn number_at(value: &Value, path: &str) -> Result<f64, Error> {
    value.as_f64().ok_or_else(|| type_error(path, "Number"))
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

/// As [`categories_of`], but an absent table is an empty list (projectiles
/// and props omit it in the installed data).
fn optional_categories_of(body: &Value) -> Result<Vec<String>, Error> {
    match body.get("Categories") {
        None | Some(Value::Nil) => Ok(Vec::new()),
        Some(_) => categories_of(body),
    }
}

fn categories_of(body: &Value) -> Result<Vec<String>, Error> {
    let list = required(body, "Categories", "Categories")?;
    if !matches!(list, Value::Table(_)) {
        return Err(type_error("Categories", "Array"));
    }
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
    })
}

fn physics_of(body: &Value) -> Result<Physics, Error> {
    let t = record_at(required(body, "Physics", "Physics")?, "Physics")?;
    Ok(Physics {
        motion_type: req_string(t, "MotionType", "Physics")?,
        max_speed: opt_number(t, "MaxSpeed", "Physics")?,
        turn_rate: opt_number(t, "TurnRate", "Physics")?,
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
        Some(v) => record_at(v, "Weapon")?,
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
            })
        })
        .collect()
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
        Kind::Projectile | Kind::Prop => {
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
