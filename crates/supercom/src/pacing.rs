//! Pacing: how many ticks a frame owes the wall clock, as a pure function.

#![expect(dead_code, reason = "wired by the code that follows the tests")]

/// The most ticks one frame steps; the rest stay owed.
pub const MAX_TICKS_PER_FRAME: u64 = 10;

/// What the clock says a frame owes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    /// The ticks to step in this frame, at most [`MAX_TICKS_PER_FRAME`].
    pub due: u64,
    /// The owed ticks the cap holds back; they stay owed.
    pub held_back: u64,
    /// The part of a tick left over, from 0 up to but not including 1.
    pub fraction: f64,
}

/// The pace at `elapsed_seconds` of wall time since the clock started, at
/// `speed` times real time, with `ticks_run` ticks already stepped. A time
/// or a speed that is not finite or is negative owes nothing.
#[must_use]
pub fn pace(elapsed_seconds: f64, speed: f64, ticks_run: u64) -> Pace {
    let _ = (elapsed_seconds, speed, ticks_run);
    Pace {
        due: 0,
        held_back: 0,
        fraction: 0.0,
    }
}

#[cfg(test)]
mod tests;
