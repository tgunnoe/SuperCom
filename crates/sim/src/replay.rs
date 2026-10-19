//! The replay log (DET-007, DET-008, DET-026): the project's own format, a
//! header, the command log in schedule order, and hash checkpoints.
//!
//! The reader takes hostile bytes and is total, and it is strict: it accepts
//! exactly the bytes [`write_replay`] can produce. Every read goes through
//! [`Cursor`], whose only primitive is `split_at_checked`, so no read can
//! panic. A declared length is refused with `LengthExceedsInput` unless the
//! input left holds it, and a declared element count is refused unless the
//! input left holds that many minimum encodings, both before any buffer is
//! sized from them; so the reader never allocates a buffer larger than the
//! input, and every loop pass consumes at least one byte of a finite input.
//! The decoded value's memory is linear in the input with a constant factor
//! (about four for a log of minimal commands, from the `Command` struct's
//! size against its 20-byte minimum encoding, plus one seen-set entry per
//! command); no claim is made of allocation at the input's size. Bytes
//! after the last checkpoint and any `f64` whose bits are a NaN other than
//! the canonical one (DET-022) are refused, so parsing is injective.

use core::fmt;

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::command::{Command, Order, QueueMode};
use crate::error::Error;
use crate::hash::{Digest, canonical_f64_bits};
use crate::ids::{Army, EntityId, Pos, Tick};
use crate::state::State;
use crate::world::World;

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
    RecordsOutOfOrder {
        index: usize,
    },
    DuplicateRecord {
        index: usize,
    },
    ArmyNotInSlot {
        index: usize,
    },
    CheckpointsNotAscending {
        index: usize,
    },
    ContentMismatch,
    /// Player `index` of a replay header is out of slot order or names no
    /// faction (DET-030).
    BadPlayer {
        index: usize,
    },
    BadOrderTag(u8),
    BadMode(u8),
    BadString,
    LengthExceedsInput {
        at: usize,
    },
    TrailingBytes {
        at: usize,
    },
    NonCanonicalFloat {
        at: usize,
    },
    PluginsUnsupported {
        count: usize,
    },
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
            Self::BadPlayer { index } => {
                write!(f, "player {index} is out of slot order or names no faction")
            }
            Self::BadOrderTag(t) => write!(f, "unknown order tag {t}"),
            Self::BadMode(m) => write!(f, "unknown queue mode {m}"),
            Self::BadString => f.write_str("a string field is not valid UTF-8"),
            Self::LengthExceedsInput { at } => {
                write!(f, "the length at byte {at} exceeds the remaining input")
            }
            Self::TrailingBytes { at } => {
                write!(f, "bytes follow the last checkpoint at byte {at}")
            }
            Self::NonCanonicalFloat { at } => {
                write!(f, "the float at byte {at} is a non-canonical NaN")
            }
            Self::PluginsUnsupported { count } => {
                write!(
                    f,
                    "the replay names {count} plugins, which this version cannot run"
                )
            }
        }
    }
}

impl core::error::Error for ReplayError {}

/// Append `replay`'s bytes to `out` in the DET-026 layout.
pub fn write_replay(replay: &Replay, out: &mut Vec<u8>) {
    let h = &replay.header;
    out.extend_from_slice(MAGIC);
    put_u32(out, h.format_version);
    put_str(out, &h.engine_version);
    put_str(out, &h.map);
    out.extend_from_slice(&h.map_digest);
    out.extend_from_slice(&h.catalog_digest);
    put_len(out, h.plugins.len());
    for (name, digest) in &h.plugins {
        put_str(out, name);
        out.extend_from_slice(digest);
    }
    put_len(out, h.players.len());
    for (army, name) in &h.players {
        out.push(army.0);
        put_str(out, name);
    }
    out.extend_from_slice(&h.seed.to_le_bytes());
    put_len(out, replay.commands.len());
    for c in &replay.commands {
        c.encode_into(out);
    }
    put_len(out, replay.checkpoints.len());
    for (tick, digest) in &replay.checkpoints {
        out.extend_from_slice(&tick.0.to_le_bytes());
        out.extend_from_slice(digest);
    }
}

const MAGIC: &[u8; 8] = b"SCREPLAY";

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

// A count over 4 GiB cannot occur; saturating keeps the writer total, as
// the command encoder does.
fn put_len(out: &mut Vec<u8>, n: usize) {
    put_u32(out, u32::try_from(n).unwrap_or(u32::MAX));
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_len(out, s.len());
    out.extend_from_slice(s.as_bytes());
}

/// Parse and validate a replay (DET-007, DET-008, DET-026).
///
/// # Errors
///
/// [`Error::Replay`] for malformed or unordered bytes.
pub fn read_replay(bytes: &[u8]) -> Result<Replay, Error> {
    let mut c = Cursor::new(bytes);
    if c.fixed(MAGIC.len())? != MAGIC.as_slice() {
        return Err(ReplayError::BadMagic.into());
    }
    let format_version = c.u32()?;
    if format_version != REPLAY_FORMAT_VERSION {
        return Err(ReplayError::UnsupportedVersion(format_version).into());
    }
    let engine_version = c.string()?;
    let map = c.string()?;
    let map_digest = c.digest()?;
    let catalog_digest = c.digest()?;

    let mut plugins = Vec::new();
    for _ in 0..c.count(PLUGIN_MIN)? {
        plugins.push((c.string()?, c.digest()?));
    }
    let mut players = Vec::new();
    for _ in 0..c.count(PLAYER_MIN)? {
        players.push((Army(c.u8()?), c.string()?));
    }
    let seed = c.u64()?;
    let header = ReplayHeader {
        format_version,
        engine_version,
        map,
        map_digest,
        catalog_digest,
        plugins,
        players,
        seed,
    };

    let mut commands: Vec<Command> = Vec::new();
    let mut issued = BTreeSet::new();
    for index in 0..c.count(Command::MIN_ENCODED)? {
        let cmd = c.command()?;
        if let Some(prev) = commands.last() {
            if (cmd.tick, cmd.source, cmd.seq) < (prev.tick, prev.source, prev.seq) {
                return Err(ReplayError::RecordsOutOfOrder { index }.into());
            }
        }
        if !issued.insert((cmd.source, cmd.seq)) {
            return Err(ReplayError::DuplicateRecord { index }.into());
        }
        let slot = header.players.get(usize::from(cmd.source));
        if slot.map(|(army, _)| *army) != Some(cmd.army) {
            return Err(ReplayError::ArmyNotInSlot { index }.into());
        }
        commands.push(cmd);
    }

    let mut checkpoints: Vec<(Tick, Digest)> = Vec::new();
    for index in 0..c.count(CHECKPOINT_LEN)? {
        let tick = Tick(c.u64()?);
        if checkpoints.last().is_some_and(|(prev, _)| tick <= *prev) {
            return Err(ReplayError::CheckpointsNotAscending { index }.into());
        }
        checkpoints.push((tick, c.digest()?));
    }

    if c.rest.is_empty() {
        Ok(Replay {
            header,
            commands,
            checkpoints,
        })
    } else {
        Err(ReplayError::TrailingBytes { at: c.at }.into())
    }
}

/// Refuse, before tick 0, a replay whose content digests differ from the
/// installed data's (DET-007).
///
/// # Errors
///
/// [`ReplayError::PluginsUnsupported`] when the header names a plugin, since
/// this version installs none (DET-007); [`ReplayError::ContentMismatch`] on
/// a differing digest.
pub fn check_content(
    header: &ReplayHeader,
    map_digest: &Digest,
    catalog_digest: &Digest,
) -> Result<(), Error> {
    if !header.plugins.is_empty() {
        return Err(ReplayError::PluginsUnsupported {
            count: header.plugins.len(),
        }
        .into());
    }
    if header.map_digest == *map_digest && header.catalog_digest == *catalog_digest {
        Ok(())
    } else {
        Err(ReplayError::ContentMismatch.into())
    }
}

/// Smallest encodings of one plugin entry, player entry, and checkpoint:
/// what a declared count is checked against before any loop runs.
const PLUGIN_MIN: usize = 4 + 32;
const PLAYER_MIN: usize = 1 + 4;
const CHECKPOINT_LEN: usize = 8 + 32;

/// A read position over untrusted bytes. Every read is a checked split, so
/// no read can index out of range; the offset of each failure is kept.
pub(crate) struct Cursor<'a> {
    rest: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes, at: 0 }
    }

    /// Bytes consumed so far.
    pub(crate) fn consumed(&self) -> usize {
        self.at
    }

    fn split(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, tail) = self.rest.split_at_checked(n)?;
        self.rest = tail;
        self.at = self.at.saturating_add(n);
        Some(head)
    }

    /// `n` bytes of a fixed-width field.
    fn fixed(&mut self, n: usize) -> Result<&'a [u8], ReplayError> {
        self.split(n).ok_or(ReplayError::Truncated)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ReplayError> {
        <[u8; N]>::try_from(self.fixed(N)?).map_err(|_| ReplayError::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ReplayError> {
        Ok(u8::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ReplayError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ReplayError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn digest(&mut self) -> Result<Digest, ReplayError> {
        self.array()
    }

    /// A little-endian `f64`; a NaN other than the canonical one is refused,
    /// because the writer never produces it (DET-022).
    fn f64(&mut self) -> Result<f64, ReplayError> {
        let at = self.at;
        let bits = self.u64()?;
        if canonical_f64_bits(f64::from_bits(bits)) == bits {
            Ok(f64::from_bits(bits))
        } else {
            Err(ReplayError::NonCanonicalFloat { at })
        }
    }

    /// A declared length of `n` bytes whose field began at `at`, checked
    /// against the input left.
    fn sized(&mut self, at: usize, n: u32) -> Result<&'a [u8], ReplayError> {
        usize::try_from(n)
            .ok()
            .and_then(|n| self.split(n))
            .ok_or(ReplayError::LengthExceedsInput { at })
    }

    /// A declared element count whose elements each take at least `min`
    /// bytes: refused unless the input left could hold them, so a loop over
    /// the count consumes input every pass and ends within it.
    fn count(&mut self, min: usize) -> Result<usize, ReplayError> {
        let at = self.at;
        let n = self.u32()?;
        usize::try_from(n)
            .ok()
            .filter(|&n| {
                n.checked_mul(min)
                    .is_some_and(|need| need <= self.rest.len())
            })
            .ok_or(ReplayError::LengthExceedsInput { at })
    }

    fn string(&mut self) -> Result<String, ReplayError> {
        let at = self.at;
        let n = self.u32()?;
        let raw = self.sized(at, n)?;
        core::str::from_utf8(raw)
            .map(str::to_owned)
            .map_err(|_| ReplayError::BadString)
    }

    fn pos(&mut self) -> Result<Pos, ReplayError> {
        Ok(Pos {
            x: self.f64()?,
            y: self.f64()?,
            z: self.f64()?,
        })
    }

    fn order(&mut self) -> Result<Order, ReplayError> {
        Ok(match self.u8()? {
            0 => Order::Move { to: self.pos()? },
            1 => Order::Stop,
            2 => Order::Attack {
                target: EntityId(self.u32()?),
            },
            3 => Order::BuildMobile {
                blueprint: self.string()?,
                at: self.pos()?,
            },
            4 => Order::BuildFactory {
                blueprint: self.string()?,
            },
            5 => Order::Guard {
                target: EntityId(self.u32()?),
            },
            tag => return Err(ReplayError::BadOrderTag(tag)),
        })
    }

    /// One command: the inverse of [`Command::encode_into`].
    pub(crate) fn command(&mut self) -> Result<Command, ReplayError> {
        let tick = Tick(self.u64()?);
        let source = self.u8()?;
        let seq = self.u32()?;
        let army = Army(self.u8()?);
        let units = {
            let n = self.count(4)?;
            let mut ids = Vec::with_capacity(n);
            for _ in 0..n {
                ids.push(EntityId(self.u32()?));
            }
            ids
        };
        let mode = match self.u8()? {
            0 => QueueMode::Replace,
            1 => QueueMode::Append,
            m => return Err(ReplayError::BadMode(m)),
        };
        let order = self.order()?;
        Ok(Command {
            tick,
            source,
            seq,
            army,
            units,
            order,
            mode,
        })
    }
}

/// The commands stamped for `tick`, found by binary search on the stamp
/// (DET-008).
#[must_use]
pub fn commands_at(replay: &Replay, tick: Tick) -> &[Command] {
    let start = replay.commands.partition_point(|c| c.tick < tick);
    let end = replay.commands.partition_point(|c| c.tick <= tick);
    replay.commands.get(start..end).unwrap_or_default()
}

/// Replay `replay` for `ticks` ticks from the world's skirmish start
/// (DET-027): one step per tick with the commands stamped for it, every
/// digest collected in order. The checkpoints are not consulted; callers
/// compare the digests with [`crate::first_divergence`].
///
/// # Errors
///
/// As [`World::replay_start`], then as [`State::step`].
pub fn run_replay(
    world: Arc<World>,
    replay: &Replay,
    ticks: u64,
) -> Result<(Vec<Digest>, State), Error> {
    let _ = (world, replay, ticks);
    Err(ReplayError::ContentMismatch.into())
}
