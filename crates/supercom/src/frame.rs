//! The window's decisions that are arithmetic or counting: whether to draw
//! at a size, and when the run is over.

use std::num::NonZeroU32;

/// The aspect ratio used before the window has a positive size.
pub const FALLBACK_ASPECT: f32 = 16.0 / 9.0;

/// Consecutive frames that may fail to present before the run gives up.
pub const MAX_STALLS: u32 = 30;

/// What to do with a frame at a window size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FramePlan {
    /// Leave the surface and the camera alone and draw nothing.
    Skip,
    /// Draw, after configuring the surface when `configure` is set and
    /// setting the camera's aspect ratio.
    Draw {
        /// Whether the size differs from the one the surface holds.
        configure: bool,
        /// Width over height.
        aspect: f32,
    },
}

/// What the loop does after a frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Keep going.
    Continue,
    /// The frame count is reached.
    Done,
    /// Too many frames in a row failed to present.
    GiveUp,
}

/// Width over height, or [`FALLBACK_ASPECT`] when a dimension is zero.
#[must_use]
pub fn aspect((width, height): (u32, u32)) -> f32 {
    if width == 0 || height == 0 {
        FALLBACK_ASPECT
    } else {
        to_f32(width) / to_f32(height)
    }
}

/// A pixel count as `f32`; window sizes are far below 2^24.
#[expect(
    clippy::cast_precision_loss,
    reason = "std has no lossless u32 to f32 conversion; sizes are a few thousand"
)]
fn to_f32(n: u32) -> f32 {
    n as f32
}

/// The plan for a frame at `size` when the surface is configured at
/// `configured`.
#[must_use]
pub fn frame_plan(size: (u32, u32), configured: Option<(u32, u32)>) -> FramePlan {
    if size.0 == 0 || size.1 == 0 {
        return FramePlan::Skip;
    }
    FramePlan::Draw {
        configure: configured != Some(size),
        aspect: aspect(size),
    }
}

/// Counts presented frames and consecutive failures.
#[derive(Clone, Debug)]
pub struct Progress {
    limit: Option<NonZeroU32>,
    presented: u32,
    stalls: u32,
}

impl Progress {
    /// A count for a run limited to `frames`, or unlimited.
    #[must_use]
    pub fn new(frames: Option<NonZeroU32>) -> Self {
        Self {
            limit: frames,
            presented: 0,
            stalls: 0,
        }
    }

    /// Records a presented frame.
    pub fn presented(&mut self) -> Step {
        self.presented = self.presented.saturating_add(1);
        self.stalls = 0;
        match self.limit {
            Some(limit) if self.presented >= limit.get() => Step::Done,
            _ => Step::Continue,
        }
    }

    /// Records a frame that did not present.
    pub fn stalled(&mut self) -> Step {
        self.stalls = self.stalls.saturating_add(1);
        if self.stalls >= MAX_STALLS {
            Step::GiveUp
        } else {
            Step::Continue
        }
    }

    /// Frames presented so far.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.presented
    }
}

#[cfg(test)]
mod tests;
