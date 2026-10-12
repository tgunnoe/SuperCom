use core::fmt;

use crate::schedule::ScheduleError;

/// Why a sim operation did not complete.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The tick counter would pass `u64::MAX`.
    TickOverflow,
    /// A command set cannot be scheduled.
    Schedule(ScheduleError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TickOverflow => f.write_str("the tick counter cannot advance past u64::MAX"),
            Self::Schedule(e) => write!(f, "cannot schedule the commands: {e}"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::TickOverflow => None,
            Self::Schedule(e) => Some(e),
        }
    }
}

impl From<ScheduleError> for Error {
    fn from(e: ScheduleError) -> Self {
        Self::Schedule(e)
    }
}
