//! The ordering rule of the command stream: which commands run in which tick,
//! and in what order within it.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

/// One player command, stamped with the tick at whose start it takes effect.
///
/// `source` is the issuing player's slot. `seq` counts that source's commands
/// across the whole match, so `(source, seq)` names a command uniquely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// The tick at whose start the command is applied.
    pub tick: u64,
    /// The issuing player's slot.
    pub source: u8,
    /// The issuing source's own command counter.
    pub seq: u32,
    /// The command body; opaque to the scheduler.
    pub payload: Vec<u8>,
}

impl Command {
    /// Append the command's canonical encoding to `out`: tick, source, seq,
    /// payload length (64 bits), then payload, all integers little-endian.
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.tick.to_le_bytes());
        out.push(self.source);
        out.extend_from_slice(&self.seq.to_le_bytes());
        let len = u64::try_from(self.payload.len()).unwrap_or(u64::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&self.payload);
    }
}

/// Commands grouped by the tick that applies them, each group in application
/// order. A `BTreeMap` iterates by ascending tick, never by hash order.
pub type Schedule = BTreeMap<u64, Vec<Command>>;

/// Why a set of commands cannot be scheduled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    /// A command is stamped for a tick that has already started.
    Late {
        /// The smallest late tick in the set.
        tick: u64,
        /// The first tick that has not started.
        next_tick: u64,
    },
    /// Two commands share a `(source, seq)` pair.
    Duplicate {
        /// The issuing source.
        source: u8,
        /// The repeated counter value.
        seq: u32,
    },
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Late { tick, next_tick } => {
                write!(
                    f,
                    "command stamped for tick {tick}, but tick {next_tick} is next"
                )
            }
            Self::Duplicate { source, seq } => {
                write!(f, "source {source} issued sequence number {seq} twice")
            }
        }
    }
}

impl Error for ScheduleError {}

/// Group `commands` by stamped tick and order each group by `(source, seq)`.
///
/// The result depends only on the set of commands, never on the order they
/// arrived in. The error reported for a bad set depends only on the set too.
///
/// # Errors
///
/// [`ScheduleError::Late`] if any command is stamped before `next_tick`, and
/// [`ScheduleError::Duplicate`] if two commands share `(source, seq)`.
pub fn schedule(commands: &[Command], next_tick: u64) -> Result<Schedule, ScheduleError> {
    let mut sorted: Vec<&Command> = commands.iter().collect();
    sorted.sort_by_key(|c| (c.tick, c.source, c.seq));

    if let Some(first) = sorted.first() {
        if first.tick < next_tick {
            return Err(ScheduleError::Late {
                tick: first.tick,
                next_tick,
            });
        }
    }

    let mut seen = BTreeSet::new();
    let mut out = Schedule::new();
    for c in sorted {
        if !seen.insert((c.source, c.seq)) {
            return Err(ScheduleError::Duplicate {
                source: c.source,
                seq: c.seq,
            });
        }
        out.entry(c.tick).or_default().push(c.clone());
    }
    Ok(out)
}

/// Six commands from four sources over three ticks, in no particular order:
/// the fixed input of the ordering and replay tests.
#[must_use]
pub fn synthetic_commands() -> Vec<Command> {
    let cmd = |tick, source, seq, payload: &[u8]| Command {
        tick,
        source,
        seq,
        payload: payload.to_vec(),
    };
    vec![
        cmd(2, 1, 0, &[0xA1]),
        cmd(2, 0, 0, &[0xB0, 0xB1]),
        cmd(2, 1, 1, &[]),
        cmd(0, 3, 0, &[0xC0]),
        cmd(5, 0, 1, &[0xD0, 0xD1, 0xD2]),
        cmd(5, 2, 0, &[0xE0]),
    ]
}
