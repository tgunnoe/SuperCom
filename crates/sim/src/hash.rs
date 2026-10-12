use crate::ids::Tick;
use crate::state::State;

/// A SHA-256 digest.
pub type Digest = [u8; 32];

/// The chain value before tick 0: thirty-two zero bytes.
pub const GENESIS: Digest = [0; 32];

/// The bit pattern of `x` with every NaN mapped to the canonical quiet NaN
/// `0x7ff8_0000_0000_0000` (DET-022); every other value keeps its bits.
#[must_use]
pub fn canonical_f64_bits(x: f64) -> u64 {
    x.to_bits()
}

/// Append the canonical serialization S of `state` to `out` (DET-009).
///
/// Field order, to be documented here by the implementation: the state's
/// scalar fields, then each collection in ascending key order. Integers are
/// little-endian, [`crate::Q`] values are their raw `i64` bits, and `f64`
/// values go through [`canonical_f64_bits`].
pub fn serialize(_state: &State, _out: &mut Vec<u8>) {}

/// One link of the chain: `SHA-256(prev || LE64(tick) || LE64(len) || state)`
/// (DET-010).
#[must_use]
pub fn chain(_prev: &Digest, _tick: Tick, _state_bytes: &[u8]) -> Digest {
    GENESIS
}

/// The earliest index at which two digest sequences differ, or `None` if they
/// are equal; a proper prefix differs at its own length (DET-011).
#[must_use]
pub fn first_divergence(_a: &[Digest], _b: &[Digest]) -> Option<usize> {
    None
}
