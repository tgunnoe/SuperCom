//! The comparison of a step with the log and the rule that ends stepping:
//! pure functions of ticks, digests, and the checkpoint list.

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
    match checkpoints.binary_search_by_key(&tick, |(at, _)| *at) {
        Err(_) => Verdict::NoCheckpoint,
        Ok(at) => match checkpoints.get(at) {
            Some((_, expected)) if expected == digest => Verdict::Match,
            _ => Verdict::Mismatch { tick },
        },
    }
}

/// Whether a step may be taken at `tick`: not past the last checkpoint's
/// tick, so a log whose last checkpoint is at L takes L + 1 steps and a log
/// with no checkpoint takes none.
#[must_use]
pub fn may_step(tick: Tick, checkpoints: &[(Tick, Digest)]) -> bool {
    checkpoints.last().is_some_and(|(last, _)| tick <= *last)
}

/// The run's summary line: ticks run, checkpoints matched out of the log's,
/// the frames at which the pacing cap held ticks back, and the unit kinds
/// drawn as markers because the run did not resolve them.
#[must_use]
pub fn summary_line(
    ticks: u64,
    matched: usize,
    total: usize,
    held_frames: u64,
    unnamed_kinds: usize,
) -> String {
    format!(
        "replayed {ticks} ticks, {matched} of {total} checkpoints matched, {held_frames} frames held back, {unnamed_kinds} kinds drawn as markers"
    )
}

#[cfg(test)]
mod tests;
