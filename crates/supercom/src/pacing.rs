//! Pacing: how many ticks a frame owes the wall clock, as a pure function.

/// The most ticks one frame steps; the rest stay owed.
pub const MAX_TICKS_PER_FRAME: u64 = 10;

/// The ticks a second of wall time owes at speed 1.
const TICKS_PER_SECOND: f64 = 10.0;

/// A whole, non-negative `f64` as a tick count; a count past `u64::MAX`
/// saturates, which the cap on a frame makes harmless.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is a floor of a non-negative number, and the cast saturates"
)]
fn to_ticks(whole: f64) -> u64 {
    whole as u64
}

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
    let nothing = Pace {
        due: 0,
        held_back: 0,
        fraction: 0.0,
    };
    let usable = |value: f64| value.is_finite() && value >= 0.0;
    if !(usable(elapsed_seconds) && usable(speed)) {
        return nothing;
    }
    let ticks = elapsed_seconds * speed * TICKS_PER_SECOND;
    let whole = ticks.floor();
    let owed = to_ticks(whole).saturating_sub(ticks_run);
    let due = owed.min(MAX_TICKS_PER_FRAME);
    Pace {
        due,
        held_back: owed - due,
        fraction: ticks - whole,
    }
}

#[cfg(test)]
mod tests;
