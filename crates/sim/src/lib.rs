//! The deterministic simulation core (docs/specs/sim.md, determinism.md).
//!
//! The sim is a pure function of its initial state and the command stream
//! (AGENTS.md R2): no clock, no operating-system randomness, no filesystem,
//! no unordered collection (I2). `docs/adr/0008-simulation-core.md` records
//! the design choices this crate makes.

mod command;
mod error;
mod hash;
mod ids;
mod rng;
mod schedule;
mod state;

pub use command::{Command, Order, QueueMode};
pub use error::Error;
pub use hash::{Digest, GENESIS, canonical_f64_bits, chain, first_divergence, serialize};
pub use ids::{Army, EntityId, Pos, Q, Tick};
pub use rng::Rng;
pub use schedule::{Schedule, ScheduleError, schedule};
pub use state::{
    ArmySnapshot, ArmyState, Counters, Entity, EntitySnapshot, PHASES, Phase, Snapshot, State, run,
};
