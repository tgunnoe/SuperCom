//! The typed v0 model of unit, projectile, and prop blueprints, and the
//! Nickel contracts that validate them.
//!
//! The contracts live in `nickel/`; [`validate`] applies one in-process
//! through `nickel-lang-core`, and the `*_from_value` functions project the
//! v0 fields of a parsed body into the typed model, ignoring every other
//! field. A field is an `Option` exactly where the installed data omits it;
//! the model never invents an engine default.
//!
//! This is the skeleton: every function returns [`Error::Unimplemented`].

use std::fmt;

use supercom_bp::Value;

/// A unit's economy block.
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
pub struct Physics {
    /// The motion type, such as `RULEUMT_Land`.
    pub motion_type: String,
    /// Maximum speed.
    pub max_speed: Option<f64>,
    /// Turn rate.
    pub turn_rate: Option<f64>,
}

/// A unit's defence block.
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
pub struct Intel {
    /// Vision radius; absent in 4 of 395 installed blocks.
    pub vision_radius: Option<f64>,
    /// Radar radius.
    pub radar_radius: Option<f64>,
}

/// One entry of a unit's weapon table.
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    /// The blueprint id.
    pub id: String,
    /// The category strings.
    pub categories: Vec<String>,
}

/// A prop blueprint's v0 fields.
#[derive(Clone, Debug, PartialEq)]
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
    /// Reserved for the skeleton: the operation is not built yet.
    Unimplemented,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract { path, reason } => write!(f, "contract violated at {path}: {reason}"),
            Self::Missing { field } => write!(f, "missing field {field}"),
            Self::Type { field, expected } => write!(f, "field {field} is not {expected}"),
            Self::Unimplemented => f.write_str("not implemented"),
        }
    }
}

impl std::error::Error for Error {}

/// Applies the contract for `kind` to `body` through `nickel-lang-core`.
///
/// # Errors
///
/// [`Error::Missing`], [`Error::Type`], or [`Error::Contract`] naming the
/// first violation.
pub fn validate(kind: Kind, body: &Value) -> Result<(), Error> {
    let _ = (kind, body);
    Err(Error::Unimplemented)
}

/// Projects the v0 fields of a unit blueprint body, ignoring the rest.
///
/// # Errors
///
/// [`Error::Missing`] or [`Error::Type`] on the first v0 field that is
/// absent or mistyped.
pub fn unit_from_value(id: &str, body: &Value) -> Result<Unit, Error> {
    let _ = (id, body);
    Err(Error::Unimplemented)
}

/// Projects the v0 fields of a projectile blueprint body.
///
/// # Errors
///
/// As [`unit_from_value`].
pub fn projectile_from_value(id: &str, body: &Value) -> Result<Projectile, Error> {
    let _ = (id, body);
    Err(Error::Unimplemented)
}

/// Projects the v0 fields of a prop blueprint body.
///
/// # Errors
///
/// As [`unit_from_value`].
pub fn prop_from_value(id: &str, body: &Value) -> Result<Prop, Error> {
    let _ = (id, body);
    Err(Error::Unimplemented)
}
