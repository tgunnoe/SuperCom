use crate::error::Error;
use crate::hash::canonical_f64_bits;
use crate::ids::{Army, EntityId, Pos, Tick};

/// Whether an order replaces the unit's queue or joins it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueMode {
    Replace,
    Append,
}

/// One order of the SIM-016 vocabulary the sim accepts so far.
#[derive(Clone, Debug, PartialEq)]
pub enum Order {
    Move { to: Pos },
    Stop,
    Attack { target: EntityId },
    BuildMobile { blueprint: String, at: Pos },
    BuildFactory { blueprint: String },
    Guard { target: EntityId },
}

fn put_pos(out: &mut Vec<u8>, p: &Pos) {
    for v in [p.x, p.y, p.z] {
        out.extend_from_slice(&canonical_f64_bits(v).to_le_bytes());
    }
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    // A name over 4 GiB cannot occur; saturating keeps the encoder total.
    let len = u32::try_from(s.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

impl Order {
    /// Append the order's canonical encoding: a tag byte, then its fields.
    ///
    /// Tags: Move 0, Stop 1, Attack 2, `BuildMobile` 3, `BuildFactory` 4,
    /// Guard 5. Entity ids are little-endian 32-bit, positions are three
    /// canonical `f64` bit patterns, strings are a little-endian 32-bit
    /// length then the bytes.
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        match self {
            Self::Move { to } => {
                out.push(0);
                put_pos(out, to);
            }
            Self::Stop => out.push(1),
            Self::Attack { target } => {
                out.push(2);
                out.extend_from_slice(&target.0.to_le_bytes());
            }
            Self::BuildMobile { blueprint, at } => {
                out.push(3);
                put_str(out, blueprint);
                put_pos(out, at);
            }
            Self::BuildFactory { blueprint } => {
                out.push(4);
                put_str(out, blueprint);
            }
            Self::Guard { target } => {
                out.push(5);
                out.extend_from_slice(&target.0.to_le_bytes());
            }
        }
    }
}

/// One player command, stamped with the tick at whose start it takes effect.
///
/// `(source, seq)` names a command uniquely: `source` is the issuing player's
/// slot and `seq` counts that source's commands across the match.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub tick: Tick,
    pub source: u8,
    pub seq: u32,
    pub army: Army,
    pub units: Vec<EntityId>,
    pub order: Order,
    pub mode: QueueMode,
}

impl Command {
    /// Append the command's canonical encoding to `out`: tick (LE64), source,
    /// seq (LE32), army, unit count (LE32) then the ids (LE32 each), mode
    /// (Replace 0, Append 1), then the order (see [`Order::encode_into`]).
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.tick.0.to_le_bytes());
        out.push(self.source);
        out.extend_from_slice(&self.seq.to_le_bytes());
        out.push(self.army.0);
        let count = u32::try_from(self.units.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&count.to_le_bytes());
        for id in &self.units {
            out.extend_from_slice(&id.0.to_le_bytes());
        }
        out.push(match self.mode {
            QueueMode::Replace => 0,
            QueueMode::Append => 1,
        });
        self.order.encode_into(out);
    }

    /// Read one command from the front of `bytes`, returning it and the
    /// bytes consumed: the inverse of [`Command::encode_into`].
    ///
    /// # Errors
    ///
    /// [`Error::Replay`] for truncated or malformed bytes.
    pub fn decode(bytes: &[u8]) -> Result<(Command, usize), Error> {
        let _ = bytes;
        Err(Error::Unimplemented)
    }
}
