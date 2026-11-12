//! Window input as the binary's own small type, and its effect on the camera.

use supercom_render::Camera;
use winit::event::MouseScrollDelta;
use winit::keyboard::KeyCode;

/// Pixels of a pixel-delta wheel that make one line.
#[expect(dead_code, reason = "the wheel conversion uses it")]
pub const PIXELS_PER_LINE: f32 = 40.0;

/// The share of the camera's distance one pan key press moves the target.
#[expect(dead_code, reason = "the pan keys use it")]
pub const PAN_FRACTION: f32 = 0.04;

/// A pan direction on the map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// Decreasing z.
    North,
    /// Increasing z.
    South,
    /// Decreasing x.
    West,
    /// Increasing x.
    East,
}

/// One input the camera or the window acts on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    /// The wheel turned; a positive line count means away from the user.
    Wheel {
        /// Lines turned, fractional for a pixel-delta wheel.
        lines: f32,
        /// Whether shift was held.
        fast: bool,
    },
    /// A pan key.
    Pan(Direction),
    /// The strategic view key.
    Strategic,
    /// The close key.
    Close,
}

/// What the window does after an input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flow {
    /// Keep running.
    Continue,
    /// Close the window.
    Close,
}

/// The input for a wheel event.
#[must_use]
pub fn wheel_input(_delta: MouseScrollDelta, fast: bool) -> Input {
    Input::Wheel { lines: 0.0, fast }
}

/// The input a physical key stands for, if any.
#[must_use]
pub fn key_input(_code: KeyCode) -> Option<Input> {
    None
}

/// Applies `input` to `camera`.
pub fn apply_input(camera: &mut Camera, input: Input) -> Flow {
    let _ = (camera, input);
    Flow::Continue
}

#[cfg(test)]
mod tests;
