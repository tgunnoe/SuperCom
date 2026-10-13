use core::fmt;

use crate::ids::Tick;
use crate::replay::ReplayError;
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
    /// A replay's bytes are refused.
    Replay(ReplayError),
    /// The static world cannot be built from the parsed values.
    Ingest(IngestError),
}

/// Why the world cannot be built (SIM-049, SIM-050, SIM-051).
#[derive(Clone, Debug, PartialEq)]
pub enum IngestError {
    UnknownMotionType(String),
    UnknownBuildRestriction(String),
    UnknownLayerKey(String),
    UnknownCapability(String),
    /// A scalar of magnitude 2^31 or more, or not finite, at `path`.
    ValueOverflow {
        path: String,
    },
    HeightmapSize {
        expected: usize,
        found: usize,
    },
    UnsupportedScale(f32),
    MissingArmyMarker(u8),
    UnknownCommander(String),
}

impl fmt::Display for IngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMotionType(v) => write!(f, "unknown motion type {v:?}"),
            Self::UnknownBuildRestriction(v) => write!(f, "unknown build restriction {v:?}"),
            Self::UnknownLayerKey(v) => write!(f, "unknown layer key {v:?}"),
            Self::UnknownCapability(v) => write!(f, "unknown command capability {v:?}"),
            Self::ValueOverflow { path } => {
                write!(f, "the value at {path} does not fit a fixed-point number")
            }
            Self::HeightmapSize { expected, found } => {
                write!(
                    f,
                    "the heightmap holds {found} samples, expected {expected}"
                )
            }
            Self::UnsupportedScale(s) => write!(
                f,
                "the heightmap scale {s} is not a power of two of at least 2^-16"
            ),
            Self::MissingArmyMarker(n) => write!(f, "the map has no marker ARMY_{n}"),
            Self::UnknownCommander(id) => write!(f, "the catalog has no commander {id}"),
        }
    }
}

impl core::error::Error for IngestError {}

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
            Self::Replay(e) => write!(f, "cannot use the replay: {e}"),
            Self::Ingest(e) => write!(f, "cannot build the world: {e}"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::TickOverflow | Self::WrongTick { .. } => None,
            Self::Schedule(e) => Some(e),
            Self::Replay(e) => Some(e),
            Self::Ingest(e) => Some(e),
        }
    }
}

impl From<ScheduleError> for Error {
    fn from(e: ScheduleError) -> Self {
        Self::Schedule(e)
    }
}

impl From<ReplayError> for Error {
    fn from(e: ReplayError) -> Self {
        Self::Replay(e)
    }
}

impl From<IngestError> for Error {
    fn from(e: IngestError) -> Self {
        Self::Ingest(e)
    }
}
