//! The replay log (DET-007, DET-008, DET-026): the project's own format, a
//! header, the command log in schedule order, and hash checkpoints.

use core::fmt;

use crate::command::Command;
use crate::error::Error;
use crate::hash::Digest;
use crate::ids::{Army, Tick};

/// The format version this crate writes and reads.
pub const REPLAY_FORMAT_VERSION: u32 = 1;

/// The initial state a replay starts from (DET-007).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayHeader {
    pub format_version: u32,
    pub engine_version: String,
    pub map: String,
    pub map_digest: Digest,
    pub catalog_digest: Digest,
    /// Each plugin's name and content digest; v0 writes none.
    pub plugins: Vec<(String, Digest)>,
    /// The player slots in order; a command's `source` indexes this list.
    pub players: Vec<(Army, String)>,
    pub seed: u64,
}

/// A whole replay: header, commands in schedule order, ascending checkpoints.
#[derive(Clone, Debug, PartialEq)]
pub struct Replay {
    pub header: ReplayHeader,
    pub commands: Vec<Command>,
    pub checkpoints: Vec<(Tick, Digest)>,
}

/// Why a replay's bytes were refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    Truncated,
    BadMagic,
    UnsupportedVersion(u32),
    RecordsOutOfOrder { index: usize },
    DuplicateRecord { index: usize },
    ArmyNotInSlot { index: usize },
    CheckpointsNotAscending { index: usize },
    ContentMismatch,
    BadOrderTag(u8),
    BadMode(u8),
    BadString,
    LengthExceedsInput { at: usize },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("the replay ends inside a field"),
            Self::BadMagic => f.write_str("the replay does not start with the SCREPLAY magic"),
            Self::UnsupportedVersion(v) => write!(f, "replay format version {v} is not supported"),
            Self::RecordsOutOfOrder { index } => {
                write!(f, "command record {index} is not after its predecessor")
            }
            Self::DuplicateRecord { index } => {
                write!(f, "command record {index} repeats an earlier (source, seq)")
            }
            Self::ArmyNotInSlot { index } => write!(
                f,
                "command record {index} names an army its source's slot does not"
            ),
            Self::CheckpointsNotAscending { index } => {
                write!(f, "checkpoint {index} is not after its predecessor")
            }
            Self::ContentMismatch => {
                f.write_str("the installed data does not match the replay's content digests")
            }
            Self::BadOrderTag(t) => write!(f, "unknown order tag {t}"),
            Self::BadMode(m) => write!(f, "unknown queue mode {m}"),
            Self::BadString => f.write_str("a string field is not valid UTF-8"),
            Self::LengthExceedsInput { at } => {
                write!(f, "the length at byte {at} exceeds the remaining input")
            }
        }
    }
}

impl core::error::Error for ReplayError {}

/// Append `replay`'s bytes to `out` in the DET-026 layout.
pub fn write_replay(replay: &Replay, out: &mut Vec<u8>) {
    let _ = (replay, out);
}

/// Parse and validate a replay (DET-007, DET-008, DET-026).
///
/// # Errors
///
/// [`Error::Replay`] for malformed or unordered bytes.
pub fn read_replay(bytes: &[u8]) -> Result<Replay, Error> {
    let _ = bytes;
    Err(Error::Unimplemented)
}

/// Refuse, before tick 0, a replay whose content digests differ from the
/// installed data's (DET-007).
///
/// # Errors
///
/// [`ReplayError::ContentMismatch`] on a differing digest.
pub fn check_content(
    header: &ReplayHeader,
    map_digest: &Digest,
    catalog_digest: &Digest,
) -> Result<(), Error> {
    let _ = (header, map_digest, catalog_digest);
    Err(Error::Unimplemented)
}

/// The commands stamped for `tick`, found by binary search on the stamp
/// (DET-008).
#[must_use]
pub fn commands_at(replay: &Replay, tick: Tick) -> &[Command] {
    let _ = (replay, tick);
    &[]
}
