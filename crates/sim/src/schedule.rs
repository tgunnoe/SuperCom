use core::fmt;
use std::collections::BTreeMap;

use crate::command::Command;
use crate::error::Error;
use crate::ids::Tick;

/// Commands grouped by the tick that applies them, each group in application
/// order.
pub type Schedule = BTreeMap<Tick, Vec<Command>>;

/// Why a set of commands cannot be scheduled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    /// A command is stamped for a tick that has already started.
    Late { tick: Tick, next_tick: Tick },
    /// Two commands share a `(source, seq)` pair.
    Duplicate { source: u8, seq: u32 },
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Late { tick, next_tick } => write!(
                f,
                "command stamped for tick {}, but tick {} is next",
                tick.0, next_tick.0
            ),
            Self::Duplicate { source, seq } => {
                write!(f, "source {source} issued sequence number {seq} twice")
            }
        }
    }
}

impl core::error::Error for ScheduleError {}

/// Group `commands` by stamped tick and order each group by `(source, seq)`
/// (DET-002), refusing late and duplicate commands (DET-003).
///
/// # Errors
///
/// [`Error::Schedule`] for a late or duplicate command;
/// [`Error::Unimplemented`] until the rule lands.
pub fn schedule(_commands: &[Command], _next_tick: Tick) -> Result<Schedule, Error> {
    Err(Error::Unimplemented)
}
