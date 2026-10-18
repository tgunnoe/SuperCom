use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use crate::build::total_work;
use crate::combat::{Projectile, ProjectileId, Scratch, WeaponState, fresh_weapons};
use crate::command::{Command, Order, QueueMode};
use crate::economy::Ledger;
use crate::error::Error;
use crate::hash::{Digest, GENESIS, chain, serialize};
use crate::ids::{Army, EntityId, Pos, Q, Tick};
use crate::rng::Rng;
use crate::schedule::{Schedule, ScheduleError};
use crate::victory::Outcome;
use crate::world::{Layer, UnitKind, World};

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
    /// The kind's id in ASCII lower case, the form the world's units are
    /// keyed by (SIM-061).
    pub blueprint: String,
    pub pos: Pos,
    pub health: Q,
    pub max_health: Q,
    /// Build work accumulated so far, in build-rate-seconds (SIM-059).
    pub built: Q,
    /// Whether the unit is finished; a spawned unit is.
    pub complete: bool,
    /// The fraction of its request granted in the last economy phase; 1
    /// when it requested nothing (SIM-043).
    pub fraction: Q,
    /// The entity it is building or assisting this tick.
    pub work: Option<EntityId>,
    /// Facing in radians in the x-z plane: 0 along +x, increasing toward
    /// +z; 0 at spawn (SIM-065).
    pub heading: f64,
    /// Speed in world units per second; 0 at spawn (SIM-065).
    pub speed: f64,
    /// The movement layer, assigned each movement phase from the kind's
    /// motion type and the terrain under the entity (SIM-064); land at
    /// spawn.
    pub layer: Layer,
    /// The cells still to visit, front first; empty when idle (SIM-068).
    pub route: Vec<(u32, u32)>,
    /// The current Move's destination; `None` when idle (SIM-068).
    pub goal: Option<Pos>,
    /// The fraction granted to the entity's maintenance request in the
    /// last economy phase; 1 when it made none (SIM-043).
    pub maintenance_fraction: Q,
    /// One state per weapon of the kind, in the kind's order; empty for a
    /// kind without weapons or an unknown blueprint (SIM-075).
    pub weapons: Vec<WeaponState>,
    pub orders: VecDeque<Order>,
}

impl Entity {
    /// Build progress in `0..=1`: 1 once complete, else the work done over
    /// the kind's build time (floored, SIM-056).
    #[must_use]
    pub fn progress(&self, kind: &UnitKind) -> Q {
        if self.complete {
            return Q::ONE;
        }
        self.built.saturating_div(total_work(kind)).min(Q::ONE)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmyState {
    pub id: Army,
    pub mass: Q,
    pub energy: Q,
    pub mass_storage: Q,
    pub energy_storage: Q,
    pub defeated: bool,
    /// The last economy phase's settlement (SIM-058).
    pub ledger: Ledger,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub commands_applied: u64,
    pub commands_ignored: u64,
    pub builds_completed: u64,
    pub stall_ticks: u64,
    /// Moves that reached their goal (SIM-068).
    pub moves_completed: u64,
    /// Projectile impacts, on a unit or the ground (SIM-078).
    pub projectile_impacts: u64,
    /// Entities removed by the death phase (SIM-080).
    pub deaths: u64,
    /// Armies defeated by the victory phase (SIM-084).
    pub defeats: u64,
    /// Matches won: 1 once the victory phase names a winner, 0 for a draw
    /// or an undecided match (SIM-085).
    pub victories: u64,
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
    /// The projectiles in flight, by id (SIM-077).
    pub projectiles: BTreeMap<ProjectileId, Projectile>,
    /// The id the next launched projectile takes; starts at 1.
    pub next_projectile_id: ProjectileId,
    /// How the match ended; `None` until the victory phase decides, and
    /// never changed afterwards (SIM-085).
    pub outcome: Option<Outcome>,
    /// How many times the route planner has been called; a diagnostic
    /// outside S, derived like the head and never serialized or hashed
    /// (SIM-068).
    pub plans: u64,
    /// The entities each army sees, rebuilt by every intel phase from the
    /// state's positions (SIM-083); outside S, derived like `plans` and
    /// never serialized or hashed (DET-009).
    pub(crate) visible: BTreeMap<Army, BTreeSet<EntityId>>,
    /// The chain value after the last completed tick; not part of the
    /// serialized state, which the chain hashes after it (DET-010).
    head: Digest,
    /// The immutable world the state was constructed with; outside S, set
    /// once at construction and never reassigned (SIM-036).
    world: Arc<World>,
}

impl State {
    /// An empty state at tick 0 whose generator is seeded from `seed`.
    #[must_use]
    pub fn new(seed: u64) -> State {
        State::with_world(Arc::new(World::empty()), seed)
    }

    /// An empty state at tick 0 in `world`, whose generator is seeded from
    /// `seed`. The resume path of a replay builds the world, checks its
    /// digests against the header ([`World::check_replay`]), and starts here.
    #[must_use]
    pub fn with_world(world: Arc<World>, seed: u64) -> State {
        State {
            tick: Tick(0),
            seed,
            rng: Rng::from_seed(seed),
            next_id: EntityId(1),
            armies: BTreeMap::new(),
            entities: BTreeMap::new(),
            counters: Counters::default(),
            projectiles: BTreeMap::new(),
            next_projectile_id: ProjectileId(1),
            outcome: None,
            plans: 0,
            visible: BTreeMap::new(),
            head: GENESIS,
            world,
        }
    }

    /// The world the state was constructed with.
    #[must_use]
    pub fn world(&self) -> &Arc<World> {
        &self.world
    }

    /// The entities `army` sees as of the last intel phase, ascending by id;
    /// empty for an unknown army (SIM-083).
    #[must_use]
    pub fn visible(&self, army: Army) -> &BTreeSet<EntityId> {
        static NONE: BTreeSet<EntityId> = BTreeSet::new();
        self.visible.get(&army).unwrap_or(&NONE)
    }

    /// Whether `army` sees entity `id` as of the last intel phase (SIM-083).
    #[must_use]
    pub fn is_visible(&self, army: Army, id: EntityId) -> bool {
        self.visible(army).contains(&id)
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
                ledger: Ledger::default(),
            },
        );
    }

    /// Spawn an entity at full `health` with the next id; ids ascend from 1.
    /// The blueprint is stored ASCII lower-cased, the one form every lookup
    /// reads (SIM-061).
    ///
    /// At saturation (`u32::MAX` spawns, which a match cannot reach) the
    /// counter stays at `u32::MAX`; the id is then already live, so this
    /// returns it without inserting and the live entity is not replaced.
    pub fn spawn(&mut self, army: Army, blueprint: &str, pos: Pos, health: Q) -> EntityId {
        let id = self.next_id;
        self.next_id = EntityId(id.0.saturating_add(1));
        if self.entities.contains_key(&id) {
            return id;
        }
        let lower = blueprint.to_ascii_lowercase();
        let built = self
            .world
            .units
            .get(&lower)
            .and_then(|k| k.build_time)
            .unwrap_or(Q::ZERO);
        let weapons = fresh_weapons(&self.world, &lower);
        self.entities.insert(
            id,
            Entity {
                id,
                army,
                blueprint: lower,
                pos,
                health,
                max_health: health,
                built,
                complete: true,
                fraction: Q::ONE,
                work: None,
                heading: 0.0,
                speed: 0.0,
                layer: Layer::Land,
                route: Vec::new(),
                goal: None,
                maintenance_fraction: Q::ONE,
                weapons,
                orders: VecDeque::new(),
            },
        );
        id
    }

    /// Spawn an unfinished entity of `blueprint` (looked up after ASCII
    /// lower-casing and stored in that form) with health 0, no work done,
    /// and the kind's maximum health; `None` and no spawn for an unknown
    /// blueprint or one without a build time (SIM-059).
    pub fn spawn_under_construction(
        &mut self,
        army: Army,
        blueprint: &str,
        pos: Pos,
    ) -> Option<EntityId> {
        let key = blueprint.to_ascii_lowercase();
        let kind = self.world.units.get(&key)?;
        // A kind with no build time has no work to finish (SIM-056).
        kind.build_time?;
        let max_health = kind.max_health;
        let weapons = vec![WeaponState::default(); kind.weapons.len()];
        let id = self.next_id;
        self.next_id = EntityId(id.0.saturating_add(1));
        if self.entities.contains_key(&id) {
            return Some(id);
        }
        self.entities.insert(
            id,
            Entity {
                id,
                army,
                blueprint: key,
                pos,
                health: Q::ZERO,
                max_health,
                built: Q::ZERO,
                complete: false,
                fraction: Q::ONE,
                work: None,
                heading: 0.0,
                speed: 0.0,
                layer: Layer::Land,
                route: Vec::new(),
                goal: None,
                maintenance_fraction: Q::ONE,
                weapons,
                orders: VecDeque::new(),
            },
        );
        Some(id)
    }

    /// Run one tick: apply `commands` in the order given (the caller passes a
    /// [`Schedule`] group, already in schedule order, and this never
    /// re-sorts), run the phases of [`PHASES`] in order, advance the tick, and
    /// return the chained digest of the end-of-tick state.
    ///
    /// # Errors
    ///
    /// [`Error::TickOverflow`] if the tick counter cannot advance;
    /// [`Error::WrongTick`] if a command is not stamped for this tick;
    /// [`Error::Schedule`] if a `(source, seq)` pair repeats in the batch.
    /// A refused tick leaves the state unchanged.
    pub fn step(&mut self, commands: &[Command]) -> Result<Digest, Error> {
        self.step_traced(commands).map(|(digest, _)| digest)
    }

    /// [`State::step`], also returning the phases visited in order. The trace
    /// does not enter the digest.
    ///
    /// # Errors
    ///
    /// The errors of [`State::step`].
    pub fn step_traced(&mut self, commands: &[Command]) -> Result<(Digest, Vec<Phase>), Error> {
        let ran = self.tick;
        let next = Tick(ran.0.checked_add(1).ok_or(Error::TickOverflow)?);
        self.check_batch(commands)?;
        let mut trace = Vec::with_capacity(PHASES.len());
        let mut scratch = Scratch::default();
        for phase in PHASES {
            trace.push(phase);
            self.run_phase(phase, commands, &mut scratch);
        }
        self.tick = next;
        let mut bytes = Vec::new();
        serialize(self, &mut bytes);
        self.head = chain(&self.head, ran, &bytes);
        Ok((self.head, trace))
    }

    /// Refuse a batch that [`crate::schedule`] would not have produced for
    /// this tick. The batch's order stays the caller's: DET-013's control
    /// needs to pass commands in a non-schedule order.
    fn check_batch(&self, commands: &[Command]) -> Result<(), Error> {
        if let Some(c) = commands.iter().find(|c| c.tick != self.tick) {
            return Err(Error::WrongTick {
                tick: c.tick,
                expected: self.tick,
            });
        }
        let mut seen = BTreeSet::new();
        for c in commands {
            if !seen.insert((c.source, c.seq)) {
                return Err(ScheduleError::Duplicate {
                    source: c.source,
                    seq: c.seq,
                }
                .into());
            }
        }
        Ok(())
    }

    /// The commands, economy, build, movement, and combat phases run their
    /// rules; the intel and victory phases are no-ops until theirs land. Each that visits entities does so in
    /// ascending id (SIM-036).
    fn run_phase(&mut self, phase: Phase, commands: &[Command], scratch: &mut Scratch) {
        match phase {
            Phase::Commands => self.apply_commands(commands),
            Phase::Economy => self.run_economy(),
            Phase::Build => self.run_build(),
            Phase::Movement => self.run_movement(),
            Phase::Weapons => self.run_weapons(),
            Phase::Projectiles => self.run_projectiles(scratch),
            Phase::Damage => self.run_damage(scratch),
            Phase::Death => self.run_death(scratch),
            Phase::Intel | Phase::Victory => {}
        }
    }

    /// Apply a command only if every unit it names exists and belongs to its
    /// army; otherwise ignore it whole, so a foreign command cannot clear or
    /// extend a queue (SIM-045).
    fn apply_commands(&mut self, commands: &[Command]) {
        for c in commands {
            let owned = !c.units.is_empty()
                && c.units
                    .iter()
                    .all(|id| self.entities.get(id).is_some_and(|e| e.army == c.army));
            if !owned {
                self.counters.commands_ignored = self.counters.commands_ignored.saturating_add(1);
                continue;
            }
            for id in &c.units {
                if let Some(e) = self.entities.get_mut(id) {
                    if c.mode == QueueMode::Replace {
                        // The old head no longer explains the work item, so
                        // the work ends here, as after a Stop (SIM-045,
                        // SIM-061); the unfinished entity stays in place.
                        e.work = None;
                        e.orders.clear();
                    }
                    e.orders.push_back(c.order.clone());
                }
            }
            self.counters.commands_applied = self.counters.commands_applied.saturating_add(1);
        }
    }

    /// The chain value after the last completed tick.
    #[must_use]
    pub fn head(&self) -> Digest {
        self.head
    }

    /// Replace the chain head, so a state rebuilt from a checkpoint extends
    /// the chain from the checkpoint's digest (the head lies outside S).
    pub fn set_head(&mut self, head: Digest) {
        self.head = head;
    }

    /// An owned view of the state for readers (SIM-046).
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.tick,
            armies: self
                .armies
                .values()
                .map(|a| ArmySnapshot {
                    id: a.id,
                    mass: a.mass,
                    energy: a.energy,
                    defeated: a.defeated,
                    mass_storage: a.mass_storage,
                    energy_storage: a.energy_storage,
                    ledger: a.ledger,
                })
                .collect(),
            entities: self
                .entities
                .values()
                .map(|e| EntitySnapshot {
                    id: e.id,
                    army: e.army,
                    pos: e.pos,
                    health: e.health,
                    max_health: e.max_health,
                    built: e.built,
                    complete: e.complete,
                    heading: e.heading,
                    speed: e.speed,
                    layer: e.layer,
                    targets: e.weapons.iter().map(|w| w.target).collect(),
                    seen_by: self
                        .visible
                        .iter()
                        .filter(|(_, set)| set.contains(&e.id))
                        .map(|(army, _)| *army)
                        .collect(),
                })
                .collect(),
            projectiles: self
                .projectiles
                .values()
                .map(|p| ProjectileSnapshot {
                    id: p.id,
                    army: p.army,
                    pos: p.pos,
                })
                .collect(),
            outcome: self.outcome,
        }
    }
}

/// Run `ticks` ticks from `state.tick`, applying `schedule`'s commands at
/// their stamped ticks, and return the digest after each.
///
/// # Errors
///
/// The first error of [`State::step`].
pub fn run(state: &mut State, schedule: &Schedule, ticks: u64) -> Result<Vec<Digest>, Error> {
    let mut digests = Vec::new();
    for _ in 0..ticks {
        let batch = schedule.get(&state.tick).map_or(&[][..], Vec::as_slice);
        digests.push(state.step(batch)?);
    }
    Ok(digests)
}

/// An owned, read-only view of the state; entities and armies ascend by id.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: Tick,
    pub armies: Vec<ArmySnapshot>,
    pub entities: Vec<EntitySnapshot>,
    /// The projectiles in flight, ascending by id.
    pub projectiles: Vec<ProjectileSnapshot>,
    /// How the match ended, if it has (SIM-085).
    pub outcome: Option<Outcome>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub army: Army,
    pub pos: Pos,
    pub health: Q,
    pub max_health: Q,
    pub built: Q,
    pub complete: bool,
    pub heading: f64,
    pub speed: f64,
    pub layer: Layer,
    /// The weapons' targets, in the kind's order.
    pub targets: Vec<Option<EntityId>>,
    /// The armies that see the entity, ascending; empty when none does
    /// (SIM-083).
    pub seen_by: Vec<Army>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileSnapshot {
    pub id: ProjectileId,
    pub army: Army,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmySnapshot {
    pub id: Army,
    pub mass: Q,
    pub energy: Q,
    pub mass_storage: Q,
    pub energy_storage: Q,
    pub defeated: bool,
    pub ledger: Ledger,
}
