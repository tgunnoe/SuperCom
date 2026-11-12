//! Window input as the binary's own small type, and its effect on the camera.

use supercom_render::{Camera, ZoomSpeed};
use winit::event::MouseScrollDelta;
use winit::keyboard::KeyCode;

/// Pixels of a pixel-delta wheel that make one line.
pub const PIXELS_PER_LINE: f32 = 40.0;

/// The share of the camera's distance one pan key press moves the target.
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
pub fn wheel_input(delta: MouseScrollDelta, fast: bool) -> Input {
    let lines = match delta {
        MouseScrollDelta::LineDelta(_, y) => y,
        MouseScrollDelta::PixelDelta(position) => narrow(position.y) / PIXELS_PER_LINE,
    };
    Input::Wheel { lines, fast }
}

/// A pixel offset as `f32`; a touchpad's offsets are far inside its range.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a pixel offset is a few thousand at most"
)]
fn narrow(pixels: f64) -> f32 {
    pixels as f32
}

/// The input a physical key stands for, if any.
#[must_use]
pub fn key_input(code: KeyCode) -> Option<Input> {
    Some(match code {
        KeyCode::KeyW | KeyCode::ArrowUp => Input::Pan(Direction::North),
        KeyCode::KeyS | KeyCode::ArrowDown => Input::Pan(Direction::South),
        KeyCode::KeyA | KeyCode::ArrowLeft => Input::Pan(Direction::West),
        KeyCode::KeyD | KeyCode::ArrowRight => Input::Pan(Direction::East),
        KeyCode::Home => Input::Strategic,
        KeyCode::Escape => Input::Close,
        _ => return None,
    })
}

/// Applies `input` to `camera`.
pub fn apply_input(camera: &mut Camera, input: Input) -> Flow {
    match input {
        Input::Wheel { lines, fast } => {
            let speed = if fast {
                ZoomSpeed::Fast
            } else {
                ZoomSpeed::Normal
            };
            // The camera's positive steps zoom out; a wheel turned away
            // from the user zooms in.
            camera.zoom(-lines, speed);
        }
        Input::Pan(direction) => {
            let step = PAN_FRACTION * camera.distance();
            let (dx, dz) = match direction {
                Direction::North => (0.0, -step),
                Direction::South => (0.0, step),
                Direction::West => (-step, 0.0),
                Direction::East => (step, 0.0),
            };
            camera.pan(dx, dz);
        }
        Input::Strategic => camera.go_strategic(),
        Input::Close => return Flow::Close,
    }
    Flow::Continue
}

#[cfg(test)]
mod tests;
