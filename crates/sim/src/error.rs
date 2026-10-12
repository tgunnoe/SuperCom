use core::fmt;

use crate::schedule::ScheduleError;

/// Why a sim operation did not complete.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The operation is a rule the skeleton has not implemented yet.
    Unimplemented,
    /// A command set cannot be scheduled.
    Schedule(ScheduleError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unimplemented => f.write_str("the operation is not implemented yet"),
            Self::Schedule(e) => write!(f, "cannot schedule the commands: {e}"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Unimplemented => None,
            Self::Schedule(e) => Some(e),
        }
    }
}

impl From<ScheduleError> for Error {
    fn from(e: ScheduleError) -> Self {
        Self::Schedule(e)
    }
}
