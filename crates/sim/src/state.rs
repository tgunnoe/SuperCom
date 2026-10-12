use std::collections::{BTreeMap, VecDeque};

use crate::command::{Command, Order};
use crate::error::Error;
use crate::hash::{Digest, GENESIS};
use crate::ids::{Army, EntityId, Pos, Q, Tick};
use crate::rng::Rng;
use crate::schedule::Schedule;

/// One phase of a tick (SIM-044).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Commands,
    Economy,
    Build,
    Movement,
    Weapons,
    Projectiles,
    Damage,
    Death,
    Intel,
    Victory,
}

/// The phases of one tick, in the order they run.
pub const PHASES: [Phase; 10] = [
    Phase::Commands,
    Phase::Economy,
    Phase::Build,
    Phase::Movement,
    Phase::Weapons,
    Phase::Projectiles,
    Phase::Damage,
    Phase::Death,
    Phase::Intel,
    Phase::Victory,
];

#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    pub id: EntityId,
    pub army: Army,
    pub blueprint: String,
    pub pos: Pos,
    pub health: Q,
    pub max_health: Q,
    pub orders: VecDeque<Order>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmyState {
    pub id: Army,
    pub mass: Q,
    pub energy: Q,
    pub mass_storage: Q,
    pub energy_storage: Q,
    pub defeated: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub commands_applied: u64,
    pub commands_ignored: u64,
}

/// The full simulation state.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub tick: Tick,
    pub seed: u64,
    pub rng: Rng,
    pub next_id: EntityId,
    pub armies: BTreeMap<Army, ArmyState>,
    pub entities: BTreeMap<EntityId, Entity>,
    pub counters: Counters,
    /// The chain value after the last completed tick; not part of the
    /// serialized state, which the chain hashes after it (DET-010).
    head: Digest,
}

impl State {
    /// An empty state at tick 0 whose generator is seeded from `seed`.
    #[must_use]
    pub fn new(seed: u64) -> State {
        State {
            tick: Tick(0),
            seed,
            rng: Rng::from_seed(seed),
            next_id: EntityId(1),
            armies: BTreeMap::new(),
            entities: BTreeMap::new(),
            counters: Counters::default(),
            head: GENESIS,
        }
    }

    /// Add an army (not defeated), replacing any army with the same slot.
    pub fn add_army(&mut self, army: Army, mass: Q, energy: Q, mass_storage: Q, energy_storage: Q) {
        self.armies.insert(
            army,
            ArmyState {
                id: army,
                mass,
                energy,
                mass_storage,
                energy_storage,
                defeated: false,
            },
        );
    }

    /// Spawn an entity at full `health` with the next id; ids ascend from 1.
    pub fn spawn(&mut self, army: Army, blueprint: &str, pos: Pos, health: Q) -> EntityId {
        let id = self.next_id;
        // Four billion spawns cannot occur in a match; saturating keeps the
        // counter total rather than wrapping onto a live id.
        self.next_id = EntityId(id.0.saturating_add(1));
        self.entities.insert(
            id,
            Entity {
                id,
                army,
                blueprint: blueprint.to_owned(),
                pos,
                health,
                max_health: health,
                orders: VecDeque::new(),
            },
        );
        id
    }

    /// Run one tick: apply `commands` (those stamped for `self.tick`, in
    /// schedule order), run the phases of [`PHASES`] in order, advance the
    /// tick, and return the chained digest of the end-of-tick state.
    ///
    /// # Errors
    ///
    /// [`Error::Unimplemented`] until the rules land.
    pub fn step(&mut self, commands: &[Command]) -> Result<Digest, Error> {
        self.step_traced(commands).map(|(digest, _)| digest)
    }

    /// [`State::step`], also returning the phases visited in order. The trace
    /// does not enter the digest.
    ///
    /// # Errors
    ///
    /// [`Error::Unimplemented`] until the rules land.
    pub fn step_traced(&mut self, _commands: &[Command]) -> Result<(Digest, Vec<Phase>), Error> {
        Err(Error::Unimplemented)
    }

    /// An owned view of the state for readers (SIM-046).
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.tick,
            armies: Vec::new(),
            entities: Vec::new(),
        }
    }
}

/// Run `ticks` ticks from `state.tick`, applying `schedule`'s commands at
/// their stamped ticks, and return the digest after each.
///
/// # Errors
///
/// The first error of [`State::step`].
pub fn run(_state: &mut State, _schedule: &Schedule, _ticks: u64) -> Result<Vec<Digest>, Error> {
    Err(Error::Unimplemented)
}

/// An owned, read-only view of the state; entities and armies ascend by id.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: Tick,
    pub armies: Vec<ArmySnapshot>,
    pub entities: Vec<EntitySnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub army: Army,
    pub pos: Pos,
    pub health: Q,
    pub max_health: Q,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmySnapshot {
    pub id: Army,
    pub mass: Q,
    pub energy: Q,
    pub defeated: bool,
}
