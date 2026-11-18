//! The comparison of a step with the log and the rule that ends stepping:
//! pure functions of ticks, digests, and the checkpoint list.

#![expect(dead_code, reason = "wired by the code that follows the tests")]

use supercom_sim::{Digest, Tick};

/// What a step's digest came to against the log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// The log holds a checkpoint at the tick and the digest equals it.
    Match,
    /// The log holds no checkpoint at the tick.
    NoCheckpoint,
    /// The log holds a checkpoint at the tick and the digest differs.
    Mismatch {
        /// The tick of the checkpoint.
        tick: Tick,
    },
}

/// Compares the digest the step run at `tick` returned with the checkpoint
/// at `tick`, under the committed rule: the checkpoint at tick t is the
/// digest of the step run while the state's tick was t.
#[must_use]
pub fn verdict(tick: Tick, digest: &Digest, checkpoints: &[(Tick, Digest)]) -> Verdict {
    let _ = (tick, digest, checkpoints);
    Verdict::NoCheckpoint
}

/// Whether a step may be taken at `tick`: not past the last checkpoint's
/// tick, so a log whose last checkpoint is at L takes L + 1 steps and a log
/// with no checkpoint takes none.
#[must_use]
pub fn may_step(tick: Tick, checkpoints: &[(Tick, Digest)]) -> bool {
    let _ = (tick, checkpoints);
    true
}

/// The run's summary line: ticks run, checkpoints matched out of the log's,
/// and the frames at which the pacing cap held ticks back.
#[must_use]
pub fn summary_line(ticks: u64, matched: usize, total: usize, held_frames: u64) -> String {
    let _ = (ticks, matched, total, held_frames);
    String::new()
}

#[cfg(test)]
mod tests;
