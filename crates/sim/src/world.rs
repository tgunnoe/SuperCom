//! The immutable world the simulation runs in: the unit and projectile kinds
//! of the blueprint catalog, the map's terrain grid and markers, and the
//! closed enumerations the blueprints' string fields are read into
//! (docs/adr/0010-ingest-and-initial-state.md, SIM-047 to SIM-051).
//!
//! The world is built from parsed values only; reading files is
//! `supercom-sim-ingest`'s job, because this crate's lint forbids the
//! filesystem. The world lies outside the serialized state S (DET-009) and is
//! identified by the two digests the replay header carries.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use fixed::types::U32F32;
use sha2::{Digest as _, Sha256};

use crate::command::Order;
use crate::error::{Error, IngestError};
use crate::hash::{Digest, GENESIS, canonical_f64_bits};
use crate::ids::{Army, Pos, Q};
use crate::replay::{ReplayHeader, check_content};
use crate::state::State;

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
    /// token a layer name without its `LAYER_` prefix. Tokens are trimmed,
    /// and the empty string is the empty set.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnknownLayerKey`] carrying the offending token as
    /// written.
    pub fn from_caps(caps: &str) -> Result<LayerSet, Error> {
        if caps.trim().is_empty() {
            return Ok(LayerSet::default());
        }
        let mut set = LayerSet::default();
        for token in caps.split('|').map(str::trim) {
            let layer = Layer::from_key(&format!("LAYER_{token}"))
                .ok_or_else(|| IngestError::UnknownLayerKey(token.to_owned()))?;
            set = set.with(layer);
        }
        Ok(set)
    }

    /// Read `BuildOnLayerCaps` keys such as `LAYER_Land`.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnknownLayerKey`] carrying the offending key.
    pub fn from_keys(keys: &[String]) -> Result<LayerSet, Error> {
        keys.iter().try_fold(LayerSet::default(), |set, key| {
            let layer =
                Layer::from_key(key).ok_or_else(|| IngestError::UnknownLayerKey(key.clone()))?;
            Ok(set.with(layer))
        })
    }

    fn with(self, layer: Layer) -> LayerSet {
        LayerSet(self.0 | 1 << (layer as u8))
    }

    fn union(self, other: LayerSet) -> LayerSet {
        LayerSet(self.0 | other.0)
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
    /// `RULEUBR_OnMassDeposit` or `RULEUBR_OnHydrocarbonDeposit`.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnknownBuildRestriction`] for any other value.
    pub fn from_rule(rule: Option<&str>) -> Result<BuildRestriction, Error> {
        match rule {
            None => Ok(BuildRestriction::None),
            Some("RULEUBR_OnMassDeposit") => Ok(BuildRestriction::OnMassDeposit),
            Some("RULEUBR_OnHydrocarbonDeposit") => Ok(BuildRestriction::OnHydrocarbonDeposit),
            Some(other) => Err(IngestError::UnknownBuildRestriction(other.to_owned()).into()),
        }
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
    #[must_use]
    pub fn from_key(key: &str) -> Option<OrderKind> {
        match key {
            "RULEUCC_Attack" => Some(OrderKind::Attack),
            "RULEUCC_CallTransport" => Some(OrderKind::CallTransport),
            "RULEUCC_Capture" => Some(OrderKind::Capture),
            "RULEUCC_Dive" => Some(OrderKind::Dive),
            "RULEUCC_Dock" => Some(OrderKind::Dock),
            "RULEUCC_Ferry" => Some(OrderKind::Ferry),
            "RULEUCC_Guard" => Some(OrderKind::Guard),
            "RULEUCC_Move" => Some(OrderKind::Move),
            "RULEUCC_Nuke" => Some(OrderKind::Nuke),
            "RULEUCC_Overcharge" => Some(OrderKind::Overcharge),
            "RULEUCC_Patrol" => Some(OrderKind::Patrol),
            "RULEUCC_Pause" => Some(OrderKind::Pause),
            "RULEUCC_Reclaim" => Some(OrderKind::Reclaim),
            "RULEUCC_Repair" => Some(OrderKind::Repair),
            "RULEUCC_RetaliateToggle" => Some(OrderKind::RetaliateToggle),
            "RULEUCC_Sacrifice" => Some(OrderKind::Sacrifice),
            "RULEUCC_SiloBuildNuke" => Some(OrderKind::SiloBuildNuke),
            "RULEUCC_SiloBuildTactical" => Some(OrderKind::SiloBuildTactical),
            "RULEUCC_Stop" => Some(OrderKind::Stop),
            "RULEUCC_Tactical" => Some(OrderKind::Tactical),
            "RULEUCC_Transport" => Some(OrderKind::Transport),
            _ => None,
        }
    }
}

/// The capabilities a unit grants (SIM-016): one bit per [`OrderKind`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OrderCaps(u32);

impl OrderCaps {
    /// Read `CommandCaps` keys such as `RULEUCC_Move`.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnknownCapability`] carrying the offending key.
    pub fn from_keys(keys: &[String]) -> Result<OrderCaps, Error> {
        keys.iter().try_fold(OrderCaps::default(), |caps, key| {
            let kind = OrderKind::from_key(key)
                .ok_or_else(|| IngestError::UnknownCapability(key.clone()))?;
            Ok(OrderCaps(caps.0 | 1 << (kind as u32)))
        })
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
    /// orders need none.
    #[must_use]
    pub fn capability(&self) -> Option<OrderKind> {
        match self {
            Order::Move { .. } => Some(OrderKind::Move),
            Order::Stop => Some(OrderKind::Stop),
            Order::Attack { .. } => Some(OrderKind::Attack),
            Order::Guard { .. } => Some(OrderKind::Guard),
            Order::BuildMobile { .. } | Order::BuildFactory { .. } => None,
        }
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
    /// derive the land and air passability grids (SIM-048): land is passable
    /// where the cell's slope is at most [`MAX_SLOPE`], air everywhere.
    ///
    /// # Errors
    ///
    /// [`IngestError::UnsupportedScale`] unless `scale` is a power of two from
    /// 2^-16 to 2^8, so that every height converts exactly to [`Q`];
    /// [`IngestError::HeightmapSize`] unless `samples` holds
    /// `(width + 1) * (height + 1)` values.
    pub fn from_heightmap(
        width: u32,
        height: u32,
        samples: &[i16],
        scale: f32,
    ) -> Result<MapGrid, Error> {
        let shift = scale_shift(scale).ok_or(IngestError::UnsupportedScale(scale))?;
        let stride = usize_of(width).saturating_add(1);
        let expected = stride.saturating_mul(usize_of(height).saturating_add(1));
        if samples.len() != expected {
            return Err(IngestError::HeightmapSize {
                expected,
                found: samples.len(),
            }
            .into());
        }
        let heights: Vec<Q> = samples
            .iter()
            .map(|s| Q::from_bits(i64::from(*s) << shift))
            .collect();
        let mut grid = MapGrid {
            width,
            height,
            heights,
            passable: BTreeMap::new(),
        };
        let cells = usize_of(width).saturating_mul(usize_of(height));
        let mut land = Vec::with_capacity(cells);
        for z in 0..height {
            for x in 0..width {
                land.push(grid.slope(x, z) <= MAX_SLOPE);
            }
        }
        grid.passable.insert(Layer::Land, land);
        grid.passable.insert(Layer::Air, vec![true; cells]);
        Ok(grid)
    }

    /// Whether cell `(x, z)` is passable on `layer`; false on an absent
    /// layer and outside the grid.
    #[must_use]
    pub fn passable(&self, layer: Layer, x: u32, z: u32) -> bool {
        if x >= self.width || z >= self.height {
            return false;
        }
        let index = usize_of(z)
            .saturating_mul(usize_of(self.width))
            .saturating_add(usize_of(x));
        self.passable
            .get(&layer)
            .and_then(|cells| cells.get(index))
            .copied()
            .unwrap_or(false)
    }

    /// The cell's slope: the maximum absolute difference between adjacent
    /// corner samples over the cell's four edges, never a diagonal (SIM-048).
    /// Zero outside the grid.
    #[must_use]
    pub fn slope(&self, x: u32, z: u32) -> Q {
        if x >= self.width || z >= self.height {
            return Q::ZERO;
        }
        let corners = [
            self.height_at(x, z),
            self.height_at(x.saturating_add(1), z),
            self.height_at(x.saturating_add(1), z.saturating_add(1)),
            self.height_at(x, z.saturating_add(1)),
        ];
        let mut slope = Q::ZERO;
        for (i, a) in corners.iter().enumerate() {
            let b = corners[(i + 1) % corners.len()];
            slope = slope.max((*a).max(b).saturating_sub((*a).min(b)));
        }
        slope
    }

    /// The corner sample at `(x, z)`; zero outside the `(width + 1) *
    /// (height + 1)` sample grid.
    #[must_use]
    pub fn height_at(&self, x: u32, z: u32) -> Q {
        if x > self.width || z > self.height {
            return Q::ZERO;
        }
        let index = usize_of(z)
            .saturating_mul(usize_of(self.width).saturating_add(1))
            .saturating_add(usize_of(x));
        self.heights.get(index).copied().unwrap_or(Q::ZERO)
    }
}

/// The steepest cell slope land units cross: `MaxSlope` 0.75 of the
/// pathfinding mask (SIM-048).
const MAX_SLOPE: Q = Q::from_bits(3 << 30);

/// A `u32` as an index; the platforms the sim builds for have `usize` of at
/// least 32 bits, and saturating keeps the helper total.
fn usize_of(v: u32) -> usize {
    usize::try_from(v).unwrap_or(usize::MAX)
}

/// The left shift that takes a stored sample to its [`Q`] bits when `scale`
/// is 2^k for k in -16..=8, else `None`. The scale arrives as `f32` and is
/// only inspected, never computed with: the sign, the mantissa, and the
/// biased exponent are read from its bits.
fn scale_shift(scale: f32) -> Option<u32> {
    let bits = scale.to_bits();
    let exponent = bits >> 23;
    // Sign clear (the exponent field with its sign bit is below 0x100) and
    // mantissa zero.
    let pure_power = bits & 0x007f_ffff == 0 && exponent < 0x100;
    if !pure_power {
        return None;
    }
    // Q has 32 fractional bits, so 2^k scales a raw sample by << (32 + k);
    // the biased exponent is 127 + k.
    exponent
        .checked_sub(127 - 16)
        .filter(|e| *e <= 24)
        .map(|e| e + 16)
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
    /// resolve by SIM-011's rule; both digests are computed.
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
        let mut kinds = BTreeMap::new();
        for unit in units {
            let kind = convert_unit(unit, units)?;
            kinds.insert(kind.id.clone(), kind);
        }
        let mut shots = BTreeMap::new();
        for projectile in projectiles {
            let kind = convert_projectile(projectile);
            shots.insert(kind.id.clone(), kind);
        }
        let catalog_digest = catalog_digest(&kinds, &shots);
        let map_digest = map_digest(&map_name, &map, &markers);
        Ok(World {
            units: kinds,
            projectiles: shots,
            map,
            markers,
            map_name,
            catalog_digest,
            map_digest,
        })
    }

    /// Refuse, before tick 0, a replay whose content digests differ from
    /// this world's (DET-007).
    ///
    /// # Errors
    ///
    /// As [`check_content`].
    pub fn check_replay(&self, header: &ReplayHeader) -> Result<(), Error> {
        check_content(header, &self.map_digest, &self.catalog_digest)
    }

    /// Start a skirmish: one army per faction, faction `i` in slot `n = i + 1`
    /// (army `n`) with its commander spawned at the marker `ARMY_n` at the
    /// marker's position and its kind's maximum health, and the army's mass
    /// and energy equal to the commander's storage, none when the commander
    /// has none (SIM-051). Slot `n` uses the first marker of kind
    /// `Army(n)`; the marker is checked before the commander.
    ///
    /// # Errors
    ///
    /// [`IngestError::MissingArmyMarker`] for a slot with no marker;
    /// [`IngestError::UnknownCommander`] for a commander the world lacks.
    pub fn skirmish(self: Arc<Self>, seed: u64, factions: &[Faction]) -> Result<State, Error> {
        let mut state = State::with_world(Arc::clone(&self), seed);
        for (index, faction) in factions.iter().enumerate() {
            let slot = u8::try_from(index + 1).map_err(|_| IngestError::ValueOverflow {
                path: format!("factions[{index}]: no army slot"),
            })?;
            let marker = self
                .markers
                .iter()
                .find(|m| m.kind == MarkerKind::Army(slot))
                .ok_or(IngestError::MissingArmyMarker(slot))?;
            let id = faction.commander();
            let kind = self
                .units
                .get(id)
                .ok_or_else(|| IngestError::UnknownCommander(id.to_owned()))?;
            let mass = kind.storage_mass.unwrap_or(Q::ZERO);
            let energy = kind.storage_energy.unwrap_or(Q::ZERO);
            state.add_army(Army(slot), mass, energy, mass, energy);
            state.spawn(Army(slot), id, marker.pos, kind.max_health);
        }
        Ok(state)
    }
}

/// A blueprint scalar as [`Q`], rounded to the nearest representable value
/// (SIM-050). A value that is not finite, or whose magnitude is 2^31 or
/// more, is refused with the field's path; so is exactly -2^31, which the
/// checked conversion would accept, because the clause bounds the magnitude.
fn to_q(value: f64, path: impl FnOnce() -> String) -> Result<Q, Error> {
    const LIMIT: f64 = 2_147_483_648.0;
    let converted = if value.is_finite() && value.abs() < LIMIT {
        Q::checked_from_num(value)
    } else {
        None
    };
    converted.ok_or_else(|| IngestError::ValueOverflow { path: path() }.into())
}

fn to_q_opt(value: Option<f64>, path: impl FnOnce() -> String) -> Result<Option<Q>, Error> {
    value.map(|v| to_q(v, path)).transpose()
}

/// A footprint extent: a finite, non-negative whole number below 2^32.
fn to_extent(value: f64, path: impl FnOnce() -> String) -> Result<u32, Error> {
    let exact = if value.is_finite() && value.fract() == 0.0 {
        U32F32::checked_from_num(value)
    } else {
        None
    };
    exact
        .and_then(|v| v.checked_to_num::<u32>())
        .ok_or_else(|| IngestError::ValueOverflow { path: path() }.into())
}

fn convert_footprint(unit: &supercom_data::Unit) -> Result<Option<(u32, u32)>, Error> {
    let Some(footprint) = &unit.footprint else {
        return Ok(None);
    };
    let (Some(x), Some(z)) = (footprint.size_x, footprint.size_z) else {
        return Ok(None);
    };
    let x = to_extent(x, || format!("unit {}: footprint.size_x", unit.id))?;
    let z = to_extent(z, || format!("unit {}: footprint.size_z", unit.id))?;
    Ok(Some((x, z)))
}

fn convert_weapon(
    unit: &str,
    index: usize,
    weapon: &supercom_data::Weapon,
) -> Result<WeaponKind, Error> {
    let path = |field: &str| format!("unit {unit}: weapons[{index}].{field}");
    let mut target_layers = LayerSet::default();
    for (_, caps) in &weapon.fire_target_layer_caps {
        target_layers = target_layers.union(LayerSet::from_caps(caps)?);
    }
    Ok(WeaponKind {
        label: weapon.label.clone(),
        damage: to_q(weapon.damage, || path("damage"))?,
        damage_radius: weapon.damage_radius,
        damage_type: weapon.damage_type.clone(),
        rate_of_fire: weapon.rate_of_fire,
        max_radius: weapon.max_radius,
        min_radius: weapon.min_radius,
        muzzle_velocity: weapon.muzzle_velocity,
        projectile_id: weapon.projectile_id.clone(),
        target_layers,
        manual_fire: weapon.manual_fire.unwrap_or(false),
    })
}

/// Whether a `BuildableCategory` token selects `target` (SIM-011): the token
/// is one of the target's category strings, or equals its id ignoring case.
fn token_selects(token: &str, target: &supercom_data::Unit) -> bool {
    target.categories.iter().any(|c| c == token) || token.eq_ignore_ascii_case(&target.id)
}

/// The ids `builder`'s expressions select: each expression is a
/// whitespace-separated intersection of tokens, an expression with no tokens
/// selects nothing, and the set is the union over the expressions.
fn buildable_set(
    builder: &supercom_data::Unit,
    catalog: &[supercom_data::Unit],
) -> BTreeSet<String> {
    let mut selected = BTreeSet::new();
    for expression in &builder.economy.buildable_category {
        let mut tokens = expression.split_whitespace().peekable();
        if tokens.peek().is_none() {
            continue;
        }
        let tokens: Vec<&str> = tokens.collect();
        for target in catalog {
            if tokens.iter().all(|t| token_selects(t, target)) {
                selected.insert(target.id.clone());
            }
        }
    }
    selected
}

fn convert_unit(
    unit: &supercom_data::Unit,
    catalog: &[supercom_data::Unit],
) -> Result<UnitKind, Error> {
    let id = &unit.id;
    let economy = &unit.economy;
    let path = |field: &str| format!("unit {id}: {field}");
    let motion = MotionType::from_rule(&unit.physics.motion_type)
        .ok_or_else(|| IngestError::UnknownMotionType(unit.physics.motion_type.clone()))?;
    let weapons = unit
        .weapons
        .iter()
        .enumerate()
        .map(|(i, w)| convert_weapon(id, i, w))
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(UnitKind {
        id: id.clone(),
        categories: unit.categories.iter().cloned().collect(),
        build_cost_mass: to_q(economy.build_cost_mass, || path("build_cost_mass"))?,
        build_cost_energy: to_q(economy.build_cost_energy, || path("build_cost_energy"))?,
        build_time: to_q_opt(economy.build_time, || path("build_time"))?,
        build_rate: to_q_opt(economy.build_rate, || path("build_rate"))?,
        production_mass: to_q_opt(economy.production_per_second_mass, || {
            path("production_per_second_mass")
        })?,
        production_energy: to_q_opt(economy.production_per_second_energy, || {
            path("production_per_second_energy")
        })?,
        storage_mass: to_q_opt(economy.storage_mass, || path("storage_mass"))?,
        storage_energy: to_q_opt(economy.storage_energy, || path("storage_energy"))?,
        maintenance_energy: to_q_opt(economy.maintenance_consumption_per_second_energy, || {
            path("maintenance_consumption_per_second_energy")
        })?,
        buildable: buildable_set(unit, catalog),
        motion,
        max_speed: unit.physics.max_speed,
        turn_rate: unit.physics.turn_rate,
        max_acceleration: unit.physics.max_acceleration,
        max_brake: unit.physics.max_brake,
        elevation: unit.physics.elevation,
        build_restriction: BuildRestriction::from_rule(unit.physics.build_restriction.as_deref())?,
        build_on: LayerSet::from_keys(&unit.physics.build_on_layer_caps)?,
        footprint: convert_footprint(unit)?,
        health: to_q(unit.defense.health, || path("health"))?,
        max_health: to_q(unit.defense.max_health, || path("max_health"))?,
        regen_rate: to_q_opt(unit.defense.regen_rate, || path("regen_rate"))?,
        armor_type: unit.defense.armor_type.clone(),
        vision_radius: unit.intel.as_ref().and_then(|i| i.vision_radius),
        command_caps: OrderCaps::from_keys(&unit.command_caps)?,
        weapons,
    })
}

fn convert_projectile(projectile: &supercom_data::Projectile) -> ProjectileKind {
    let physics = &projectile.physics;
    ProjectileKind {
        id: projectile.id.clone(),
        categories: projectile.categories.iter().cloned().collect(),
        initial_speed: physics.initial_speed,
        max_speed: physics.max_speed,
        acceleration: physics.acceleration,
        turn_rate: physics.turn_rate,
        lifetime: physics.lifetime,
        track_target: physics.track_target.unwrap_or(false),
        use_gravity: physics.use_gravity.unwrap_or(false),
        destroy_on_water: physics.destroy_on_water.unwrap_or(false),
    }
}

/// The byte encoder of the two digests: every primitive of the documented
/// layouts, streamed into SHA-256.
struct Encoder(Sha256);

impl Encoder {
    fn new(tag: &str) -> Encoder {
        let mut e = Encoder(Sha256::new());
        e.str(tag);
        e
    }

    fn u8(&mut self, v: u8) {
        self.0.update([v]);
    }

    fn u32(&mut self, v: u32) {
        self.0.update(v.to_le_bytes());
    }

    /// A length as `u32`; a collection over four billion entries cannot
    /// occur, and saturating keeps the encoder total.
    fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).unwrap_or(u32::MAX));
    }

    fn q(&mut self, v: Q) {
        self.0.update(v.to_bits().to_le_bytes());
    }

    fn f64(&mut self, v: f64) {
        self.0.update(canonical_f64_bits(v).to_le_bytes());
    }

    fn str(&mut self, v: &str) {
        self.len(v.len());
        self.0.update(v.as_bytes());
    }

    fn option<T>(&mut self, v: Option<T>, put: impl FnOnce(&mut Encoder, T)) {
        match v {
            None => self.u8(0),
            Some(v) => {
                self.u8(1);
                put(self, v);
            }
        }
    }

    fn set(&mut self, v: &BTreeSet<String>) {
        self.len(v.len());
        for s in v {
            self.str(s);
        }
    }

    fn finish(self) -> Digest {
        self.0.finalize().into()
    }

    fn weapon(&mut self, w: &WeaponKind) {
        self.option(w.label.as_deref(), Encoder::str);
        self.q(w.damage);
        self.option(w.damage_radius, Encoder::f64);
        self.str(&w.damage_type);
        for v in [
            w.rate_of_fire,
            w.max_radius,
            w.min_radius,
            w.muzzle_velocity,
        ] {
            self.option(v, Encoder::f64);
        }
        self.option(w.projectile_id.as_deref(), Encoder::str);
        self.u8(w.target_layers.bits());
        self.u8(u8::from(w.manual_fire));
    }

    fn unit(&mut self, u: &UnitKind) {
        self.str(&u.id);
        self.set(&u.categories);
        self.q(u.build_cost_mass);
        self.q(u.build_cost_energy);
        for v in [
            u.build_time,
            u.build_rate,
            u.production_mass,
            u.production_energy,
            u.storage_mass,
            u.storage_energy,
            u.maintenance_energy,
        ] {
            self.option(v, Encoder::q);
        }
        self.set(&u.buildable);
        self.u8(u.motion as u8);
        for v in [
            u.max_speed,
            u.turn_rate,
            u.max_acceleration,
            u.max_brake,
            u.elevation,
        ] {
            self.option(v, Encoder::f64);
        }
        self.u8(u.build_restriction as u8);
        self.u8(u.build_on.bits());
        self.option(u.footprint, |e, (x, z)| {
            e.u32(x);
            e.u32(z);
        });
        self.q(u.health);
        self.q(u.max_health);
        self.option(u.regen_rate, Encoder::q);
        self.str(&u.armor_type);
        self.option(u.vision_radius, Encoder::f64);
        self.u32(u.command_caps.bits());
        self.len(u.weapons.len());
        for w in &u.weapons {
            self.weapon(w);
        }
    }

    fn projectile(&mut self, p: &ProjectileKind) {
        self.str(&p.id);
        self.set(&p.categories);
        for v in [
            p.initial_speed,
            p.max_speed,
            p.acceleration,
            p.turn_rate,
            p.lifetime,
        ] {
            self.option(v, Encoder::f64);
        }
        for flag in [p.track_target, p.use_gravity, p.destroy_on_water] {
            self.u8(u8::from(flag));
        }
    }

    fn marker(&mut self, m: &Marker) {
        self.str(&m.name);
        match m.kind {
            MarkerKind::Mass => self.u8(0),
            MarkerKind::Hydrocarbon => self.u8(1),
            MarkerKind::Army(slot) => {
                self.u8(2);
                self.u8(slot);
            }
            MarkerKind::Other => self.u8(3),
        }
        for v in [m.pos.x, m.pos.y, m.pos.z] {
            self.f64(v);
        }
    }
}

/// The catalog digest: SHA-256 over the encoding below of the unit and
/// projectile kinds in id order.
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
    let mut e = Encoder::new("catalog-v1");
    e.len(units.len());
    for unit in units.values() {
        e.unit(unit);
    }
    e.len(projectiles.len());
    for projectile in projectiles.values() {
        e.projectile(projectile);
    }
    e.finish()
}

/// The map digest: SHA-256 over the encoding below. The primitives are those of [`catalog_digest`].
///
/// Layout: the string `map-v1`; the map name as a string; `width` then
/// `height` as `u32`; every height sample's raw `i64` bits in order; the
/// markers as a `u32` count then each in input order as its name (string),
/// kind tag (`u8`: Mass 0, Hydrocarbon 1, Army 2 followed by the slot as a
/// `u8`, Other 3), and `x`, `y`, `z` as canonical `f64` bits.
#[must_use]
pub fn map_digest(map_name: &str, map: &MapGrid, markers: &[Marker]) -> Digest {
    let mut e = Encoder::new("map-v1");
    e.str(map_name);
    e.u32(map.width);
    e.u32(map.height);
    for h in &map.heights {
        e.q(*h);
    }
    e.len(markers.len());
    for marker in markers {
        e.marker(marker);
    }
    e.finish()
}
