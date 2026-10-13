//! The immutable world the simulation runs in: the unit and projectile kinds
//! of the blueprint catalog, the map's terrain grid and markers, and the
//! closed enumerations the blueprints' string fields are read into
//! (docs/adr/0010-ingest-and-initial-state.md, SIM-047 to SIM-051).
//!
//! The world is built from parsed values only; reading files is
//! `supercom-sim-ingest`'s job, because this crate's lint forbids the
//! filesystem. The world lies outside the serialized state S (DET-009) and is
//! identified by the two digests the replay header carries.
//!
//! Every constructor and query below that carries "stub" in its documentation
//! is a placeholder awaiting its implementation; the rules it will carry are
//! the clauses cited at the item.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::command::Order;
use crate::error::{Error, IngestError};
use crate::hash::{Digest, GENESIS};
use crate::ids::{Pos, Q};
use crate::state::State;

/// The error every stub returns until its implementation lands.
fn stub() -> Error {
    Error::Ingest(IngestError::ValueOverflow {
        path: String::from("stub: not implemented"),
    })
}

/// A movement or placement layer (SIM-030), in the order of `Ord`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Air,
    Land,
    Orbit,
    Seabed,
    Sub,
    Water,
}

impl Layer {
    /// The layer named by a `BuildOnLayerCaps` key such as `LAYER_Land`.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Layer> {
        match key {
            "LAYER_Air" => Some(Layer::Air),
            "LAYER_Land" => Some(Layer::Land),
            "LAYER_Orbit" => Some(Layer::Orbit),
            "LAYER_Seabed" => Some(Layer::Seabed),
            "LAYER_Sub" => Some(Layer::Sub),
            "LAYER_Water" => Some(Layer::Water),
            _ => None,
        }
    }
}

/// A set of layers (SIM-049): one bit per [`Layer`], in `Ord` order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerSet(u8);

impl LayerSet {
    /// Read a `|`-separated caps string such as `Land|Water|Seabed`, each
    /// token a layer name without its `LAYER_` prefix. Stub.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnknownLayerKey`] carrying the offending token as
    /// written.
    pub fn from_caps(caps: &str) -> Result<LayerSet, Error> {
        let _ = caps;
        Err(stub())
    }

    /// Read `BuildOnLayerCaps` keys such as `LAYER_Land`. Stub.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnknownLayerKey`] carrying the offending key.
    pub fn from_keys(keys: &[String]) -> Result<LayerSet, Error> {
        let _ = keys;
        Err(stub())
    }

    /// Whether `layer` is in the set.
    #[must_use]
    pub fn contains(self, layer: Layer) -> bool {
        self.0 >> (layer as u8) & 1 == 1
    }

    /// The layers in the set, ascending.
    pub fn iter(self) -> impl Iterator<Item = Layer> {
        [
            Layer::Air,
            Layer::Land,
            Layer::Orbit,
            Layer::Seabed,
            Layer::Sub,
            Layer::Water,
        ]
        .into_iter()
        .filter(move |l| self.contains(*l))
    }

    /// The set's bit pattern: bit `n` is the layer with discriminant `n`.
    #[must_use]
    pub fn bits(self) -> u8 {
        self.0
    }
}

/// A `Physics.MotionType` (SIM-029), in the order the digest encodes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionType {
    #[default]
    None,
    Air,
    Land,
    Water,
    Hover,
    Amphibious,
    SurfacingSub,
    AmphibiousFloating,
}

impl MotionType {
    /// The motion type named by a rule string such as `RULEUMT_Land`.
    #[must_use]
    pub fn from_rule(rule: &str) -> Option<MotionType> {
        match rule {
            "RULEUMT_None" => Some(MotionType::None),
            "RULEUMT_Air" => Some(MotionType::Air),
            "RULEUMT_Land" => Some(MotionType::Land),
            "RULEUMT_Water" => Some(MotionType::Water),
            "RULEUMT_Hover" => Some(MotionType::Hover),
            "RULEUMT_Amphibious" => Some(MotionType::Amphibious),
            "RULEUMT_SurfacingSub" => Some(MotionType::SurfacingSub),
            "RULEUMT_AmphibiousFloating" => Some(MotionType::AmphibiousFloating),
            _ => None,
        }
    }
}

/// A `Physics.BuildRestriction` (SIM-049), in the order the digest encodes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildRestriction {
    #[default]
    None,
    OnMassDeposit,
    OnHydrocarbonDeposit,
}

impl BuildRestriction {
    /// Read the field: `None` for an absent field, the restriction for
    /// `RULEUBR_OnMassDeposit` or `RULEUBR_OnHydrocarbonDeposit`. Stub.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnknownBuildRestriction`] for any other value.
    pub fn from_rule(rule: Option<&str>) -> Result<BuildRestriction, Error> {
        let _ = rule;
        Err(stub())
    }
}

/// One of the 21 command capabilities of SIM-016, in the order of `Ord`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OrderKind {
    Attack,
    CallTransport,
    Capture,
    Dive,
    Dock,
    Ferry,
    Guard,
    Move,
    Nuke,
    Overcharge,
    Patrol,
    Pause,
    Reclaim,
    Repair,
    RetaliateToggle,
    Sacrifice,
    SiloBuildNuke,
    SiloBuildTactical,
    Stop,
    Tactical,
    Transport,
}

impl OrderKind {
    /// The capability named by a `CommandCaps` key such as `RULEUCC_Move`.
    /// Stub.
    #[must_use]
    pub fn from_key(key: &str) -> Option<OrderKind> {
        let _ = key;
        None
    }
}

/// The capabilities a unit grants (SIM-016): one bit per [`OrderKind`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OrderCaps(u32);

impl OrderCaps {
    /// Read `CommandCaps` keys such as `RULEUCC_Move`. Stub.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnknownCapability`] carrying the offending key.
    pub fn from_keys(keys: &[String]) -> Result<OrderCaps, Error> {
        let _ = keys;
        Err(stub())
    }

    /// Whether the set grants `kind`.
    #[must_use]
    pub fn allows(self, kind: OrderKind) -> bool {
        self.0 >> (kind as u32) & 1 == 1
    }

    /// The set's bit pattern: bit `n` is the capability with discriminant `n`.
    #[must_use]
    pub fn bits(self) -> u32 {
        self.0
    }
}

impl Order {
    /// The capability a unit needs to accept the order. Building is gated by
    /// the builder's buildable set rather than a capability, so the two build
    /// orders need none. Stub.
    #[must_use]
    pub fn capability(&self) -> Option<OrderKind> {
        None
    }
}

/// One weapon of a unit kind.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponKind {
    pub label: Option<String>,
    pub damage: Q,
    pub damage_radius: Option<f64>,
    pub damage_type: String,
    pub rate_of_fire: Option<f64>,
    pub max_radius: Option<f64>,
    pub min_radius: Option<f64>,
    pub muzzle_velocity: Option<f64>,
    pub projectile_id: Option<String>,
    /// The union of the values of `FireTargetLayerCapsTable`.
    pub target_layers: LayerSet,
    pub manual_fire: bool,
}

/// A unit blueprint as the sim holds it: scalars that enter state arithmetic
/// are [`Q`] (SIM-050), geometry stays `f64`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitKind {
    pub id: String,
    pub categories: BTreeSet<String>,
    pub build_cost_mass: Q,
    pub build_cost_energy: Q,
    pub build_time: Option<Q>,
    pub build_rate: Option<Q>,
    pub production_mass: Option<Q>,
    pub production_energy: Option<Q>,
    pub storage_mass: Option<Q>,
    pub storage_energy: Option<Q>,
    pub maintenance_energy: Option<Q>,
    /// The blueprint ids this unit can build, resolved once at ingest by
    /// SIM-011's rule.
    pub buildable: BTreeSet<String>,
    pub motion: MotionType,
    pub max_speed: Option<f64>,
    pub turn_rate: Option<f64>,
    pub max_acceleration: Option<f64>,
    pub max_brake: Option<f64>,
    pub elevation: Option<f64>,
    pub build_restriction: BuildRestriction,
    pub build_on: LayerSet,
    pub footprint: Option<(u32, u32)>,
    pub health: Q,
    pub max_health: Q,
    pub regen_rate: Option<Q>,
    pub armor_type: String,
    pub vision_radius: Option<f64>,
    pub command_caps: OrderCaps,
    pub weapons: Vec<WeaponKind>,
}

/// A projectile blueprint as the sim holds it. A flag the data omits reads
/// as false, because a flag the data omits is not set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectileKind {
    pub id: String,
    pub categories: BTreeSet<String>,
    pub initial_speed: Option<f64>,
    pub max_speed: Option<f64>,
    pub acceleration: Option<f64>,
    pub turn_rate: Option<f64>,
    pub lifetime: Option<f64>,
    pub track_target: bool,
    pub use_gravity: bool,
    pub destroy_on_water: bool,
}

/// What a scenario marker marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerKind {
    Mass,
    Hydrocarbon,
    /// The start position of army slot `n`, named `ARMY_n`.
    Army(u8),
    Other,
}

/// A named point of the scenario save file.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub name: String,
    pub kind: MarkerKind,
    pub pos: Pos,
}

/// The map's terrain: heights and the per-layer passability grids (SIM-033,
/// SIM-048).
#[derive(Clone, Debug, PartialEq)]
pub struct MapGrid {
    /// Width in cells.
    pub width: u32,
    /// Height in cells.
    pub height: u32,
    /// `(width + 1) * (height + 1)` corner samples, row by row, each the
    /// stored `i16` times the heightmap scale, exactly.
    pub heights: Vec<Q>,
    /// `width * height` cells per layer, row by row. Land and Air are present
    /// after ingest; the other layers are absent until a clause derives them,
    /// and a query on an absent layer is false.
    pub passable: BTreeMap<Layer, Vec<bool>>,
}

impl MapGrid {
    /// Build the grid from a heightmap's `i16` samples and its scale, and
    /// derive the land and air passability grids (SIM-048). Stub.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnsupportedScale`] unless `scale` is a power of two no
    /// smaller than 2^-16, so that every height converts exactly to [`Q`];
    /// [`IngestError::HeightmapSize`] unless `samples` holds
    /// `(width + 1) * (height + 1)` values.
    pub fn from_heightmap(
        width: u32,
        height: u32,
        samples: &[i16],
        scale: f32,
    ) -> Result<MapGrid, Error> {
        let _ = (width, height, samples, scale);
        Err(stub())
    }

    /// Whether cell `(x, z)` is passable on `layer`; false on an absent
    /// layer. Stub.
    #[must_use]
    pub fn passable(&self, layer: Layer, x: u32, z: u32) -> bool {
        let _ = (layer, x, z);
        false
    }

    /// The cell's slope: the maximum absolute difference between adjacent
    /// corner samples over the cell's four edges, never a diagonal (SIM-048).
    /// Stub.
    #[must_use]
    pub fn slope(&self, x: u32, z: u32) -> Q {
        let _ = (x, z);
        Q::ZERO
    }

    /// The corner sample at `(x, z)`. Stub.
    #[must_use]
    pub fn height_at(&self, x: u32, z: u32) -> Q {
        let _ = (x, z);
        Q::ZERO
    }
}

/// The four playable factions and their commanders (SIM-035).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    UEF,
    Aeon,
    Cybran,
    Seraphim,
}

impl Faction {
    /// The blueprint id of the faction's commander.
    #[must_use]
    pub fn commander(self) -> &'static str {
        match self {
            Faction::UEF => "UEL0001",
            Faction::Aeon => "UAL0001",
            Faction::Cybran => "URL0001",
            Faction::Seraphim => "XSL0001",
        }
    }
}

/// The immutable world of a match (SIM-049 to SIM-051).
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub units: BTreeMap<String, UnitKind>,
    pub projectiles: BTreeMap<String, ProjectileKind>,
    pub map: MapGrid,
    pub markers: Vec<Marker>,
    pub map_name: String,
    pub catalog_digest: Digest,
    pub map_digest: Digest,
}

impl World {
    /// A world with no kinds, no terrain, and no markers; both digests are
    /// the all-zero value, which names no world.
    #[must_use]
    pub fn empty() -> World {
        World {
            units: BTreeMap::new(),
            projectiles: BTreeMap::new(),
            map: MapGrid {
                width: 0,
                height: 0,
                heights: Vec::new(),
                passable: BTreeMap::new(),
            },
            markers: Vec::new(),
            map_name: String::new(),
            catalog_digest: GENESIS,
            map_digest: GENESIS,
        }
    }

    /// Build the world from parsed values: every blueprint scalar the sim
    /// holds as [`Q`] converts once through `Q::checked_from_num`, rounding
    /// to the nearest representable value (SIM-050); the buildable sets
    /// resolve by SIM-011's rule; both digests are computed. Stub.
    ///
    /// # Errors
    ///
    /// [`IngestError::ValueOverflow`] naming the field's path for a value of
    /// magnitude 2^31 or more, or not finite; the other [`IngestError`]
    /// variants for a value outside a closed set.
    pub fn from_parts(
        units: &[supercom_data::Unit],
        projectiles: &[supercom_data::Projectile],
        map: MapGrid,
        markers: Vec<Marker>,
        map_name: String,
    ) -> Result<World, Error> {
        let _ = (units, projectiles, map, markers, map_name);
        Err(stub())
    }

    /// Start a skirmish: one army per faction, faction `i` in slot `n = i + 1`
    /// (army `n`) with its commander spawned at the marker `ARMY_n` at the
    /// marker's position and its kind's maximum health, and the army's mass
    /// and energy equal to the commander's storage (SIM-051). Stub.
    ///
    /// # Errors
    ///
    /// [`IngestError::MissingArmyMarker`] for a slot with no marker;
    /// [`IngestError::UnknownCommander`] for a commander the world lacks.
    pub fn skirmish(self: Arc<Self>, seed: u64, factions: &[Faction]) -> Result<State, Error> {
        let _ = (seed, factions);
        Err(stub())
    }
}

/// The catalog digest: SHA-256 over the encoding below of the unit and
/// projectile kinds in id order. Stub (returns the all-zero value).
///
/// Primitives: `u8`; `u32` and `i64` little-endian; a [`Q`] as its raw `i64`
/// bits; an `f64` as its [`crate::canonical_f64_bits`], little-endian; a
/// string as a `u32` byte length then its UTF-8 bytes; `Option<T>` as a `u8`
/// 0, or a `u8` 1 then the `T`; a set of strings as a `u32` count then the
/// strings ascending; a sequence as a `u32` count then its items in order.
///
/// Layout: the string `catalog-v1`; the units as a `u32` count then each
/// [`UnitKind`] in id order with its fields in declaration order, where
/// `motion` and `build_restriction` are their declaration index as a `u8`,
/// `build_on` is [`LayerSet::bits`], `command_caps` is [`OrderCaps::bits`],
/// `footprint` is an option of two `u32`, and `weapons` is a sequence of
/// [`WeaponKind`] with its fields in declaration order (`target_layers` as
/// [`LayerSet::bits`], `manual_fire` as a `u8`); then the projectiles as a
/// `u32` count then each [`ProjectileKind`] in id order with its fields in
/// declaration order (the three flags as a `u8` each).
#[must_use]
pub fn catalog_digest(
    units: &BTreeMap<String, UnitKind>,
    projectiles: &BTreeMap<String, ProjectileKind>,
) -> Digest {
    let _ = (units, projectiles);
    GENESIS
}

/// The map digest: SHA-256 over the encoding below. Stub (returns the
/// all-zero value). The primitives are those of [`catalog_digest`].
///
/// Layout: the string `map-v1`; the map name as a string; `width` then
/// `height` as `u32`; every height sample's raw `i64` bits in order; the
/// markers as a `u32` count then each in input order as its name (string),
/// kind tag (`u8`: Mass 0, Hydrocarbon 1, Army 2 followed by the slot as a
/// `u8`, Other 3), and `x`, `y`, `z` as canonical `f64` bits.
#[must_use]
pub fn map_digest(map_name: &str, map: &MapGrid, markers: &[Marker]) -> Digest {
    let _ = (map_name, map, markers);
    GENESIS
}
