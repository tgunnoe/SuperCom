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
/// Field order: tick (LE64), seed (LE64), the generator state (LE64), next
/// entity id (LE32), commands applied (LE64), commands ignored (LE64); the
/// armies, a LE32 count then each in ascending slot order as slot (u8), mass,
/// energy, mass storage, energy storage (each the raw `i64` bits of its
/// [`crate::Q`], LE64), defeated (u8, 0 or 1); the entities, a LE32 count then
/// each in ascending id order as id (LE32), army (u8), blueprint (LE32 length,
/// bytes), x, y, z (each [`canonical_f64_bits`], LE64), health and max health
/// (raw `i64` bits, LE64), then the queued orders (LE32 count, each in
/// [`crate::Order::encode_into`]'s encoding, front to back). The chain head is
/// not part of S.
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
