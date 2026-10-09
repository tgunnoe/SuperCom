//! The per-tick state hash chain and the desync report built on it.

use std::fmt::Write as _;

use sha2::{Digest as _, Sha256};

use crate::command::Schedule;

/// A SHA-256 digest.
pub type Digest = [u8; 32];

/// The chain value before tick 0: thirty-two zero bytes.
pub const GENESIS: Digest = [0; 32];

/// One link: `SHA-256(prev || tick || len(state) || state)`, with `tick` and
/// `len(state)` as 64-bit little-endian integers.
#[must_use]
pub fn chain(prev: &Digest, tick: u64, state: &[u8]) -> Digest {
    let mut h = Sha256::new();
    h.update(prev);
    h.update(tick.to_le_bytes());
    h.update(u64::try_from(state.len()).unwrap_or(u64::MAX).to_le_bytes());
    h.update(state);
    h.finalize().into()
}

/// The chain over a run: element `t` is the hash after tick `t`, taken from
/// the state `states[t]` at the end of that tick.
#[must_use]
pub fn chain_all(states: &[Vec<u8>]) -> Vec<Digest> {
    let mut prev = GENESIS;
    let mut out = Vec::with_capacity(states.len());
    for (tick, state) in (0u64..).zip(states) {
        prev = chain(&prev, tick, state);
        out.push(prev);
    }
    out
}

/// The earliest tick at which two hash sequences differ, or `None` if they
/// are equal. A sequence that is a proper prefix of the other differs at its
/// own length.
#[must_use]
pub fn first_divergence(a: &[Digest], b: &[Digest]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .or_else(|| (a.len() != b.len()).then_some(a.len().min(b.len())))
}

/// Lower-case hexadecimal text of a digest.
#[must_use]
pub fn hex(d: &Digest) -> String {
    let mut s = String::with_capacity(64);
    for b in d {
        // Writing to a `String` cannot fail.
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// A synthetic run of `ticks` states: integer arithmetic only, with a state
/// length that varies from 8 to 32 bytes.
#[must_use]
pub fn synthetic_states(ticks: u64) -> Vec<Vec<u8>> {
    (0..ticks)
        .map(|t| {
            let mut x = t.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let mut s = Vec::new();
            for _ in 0..=(t % 4) {
                s.extend_from_slice(&x.to_le_bytes());
                x = (x.rotate_left(13) ^ t).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            }
            s
        })
        .collect()
}

/// The hash chain of a toy run over `schedule`: the state at the end of tick
/// `t` is the encoding of every command applied through `t`, in application
/// order. It stands in for a simulation whose state depends on its inputs and
/// on their order, and for nothing more.
#[must_use]
pub fn scheduled_run(schedule: &Schedule, ticks: u64) -> Vec<Digest> {
    let mut applied = Vec::new();
    let mut prev = GENESIS;
    let mut out = Vec::new();
    for tick in 0..ticks {
        for c in schedule.get(&tick).into_iter().flatten() {
            c.encode_into(&mut applied);
        }
        prev = chain(&prev, tick, &applied);
        out.push(prev);
    }
    out
}
