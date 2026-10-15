//! The deterministic simulation core (docs/specs/sim.md, determinism.md).
//!
//! The sim is a pure function of its initial state and the command stream
//! (AGENTS.md R2): no clock, no operating-system randomness, no filesystem,
//! no unordered collection (I2). `docs/adr/0008-simulation-core.md` records
//! the design choices this crate makes.

mod build;
mod combat;
mod command;
mod economy;
mod error;
mod hash;
mod ids;
mod movement;
mod planner;
mod replay;
mod rng;
mod schedule;
mod state;
mod world;

pub use build::{BUILD_RANGE, BUILD_TIME_FLOOR, BuildRates, build_rates, footprint_of, total_work};
pub use combat::{
    GRAVITY, IMPACT_RADIUS, Projectile, ProjectileId, WeaponState, cooldown_ticks, distance_xz,
};
pub use command::{Command, Order, QueueMode};
pub use economy::{Ledger, per_tick};
pub use error::{Error, IngestError};
pub use hash::{Digest, GENESIS, canonical_f64_bits, chain, first_divergence, serialize};
pub use ids::{Army, EntityId, Pos, Q, Tick};
pub use movement::{ARRIVAL_RADIUS, FOLLOW_RANGE, TICK_SECONDS, cell_centre, cell_of};
pub use planner::{plan_route, plan_route_expansions};
pub use replay::{
    REPLAY_FORMAT_VERSION, Replay, ReplayError, ReplayHeader, check_content, commands_at,
    read_replay, write_replay,
};
pub use rng::Rng;
pub use schedule::{Schedule, ScheduleError, schedule};
pub use state::{
    ArmySnapshot, ArmyState, Counters, Entity, EntitySnapshot, PHASES, Phase, ProjectileSnapshot,
    Snapshot, State, run,
};
pub use world::{
    BuildRestriction, Faction, Layer, LayerSet, MapGrid, Marker, MarkerKind, MotionType, OrderCaps,
    OrderKind, ProjectileKind, UnitKind, WeaponKind, World, catalog_digest, map_digest,
};
