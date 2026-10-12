use core::fmt;

use crate::ids::Tick;
use crate::schedule::ScheduleError;

/// Why a sim operation did not complete.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The tick counter would pass `u64::MAX`.
    TickOverflow,
    /// A command is stamped for a tick other than the one being run.
    WrongTick { tick: Tick, expected: Tick },
    /// A command set cannot be scheduled.
    Schedule(ScheduleError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TickOverflow => f.write_str("the tick counter cannot advance past u64::MAX"),
            Self::WrongTick { tick, expected } => write!(
                f,
                "command stamped for tick {}, but tick {} is being run",
                tick.0, expected.0
            ),
            Self::Schedule(e) => write!(f, "cannot schedule the commands: {e}"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::TickOverflow | Self::WrongTick { .. } => None,
            Self::Schedule(e) => Some(e),
        }
    }
}

impl From<ScheduleError> for Error {
    fn from(e: ScheduleError) -> Self {
        Self::Schedule(e)
    }
}
