//! The four marker models: flat shapes in the plane y = 0.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "wired by the code that follows the tests")
)]

use supercom_render::Model;

/// The model of a kind that does not move.
pub const STATIC_MODEL: usize = 0;
/// The model of a mobile unit.
pub const MOBILE_MODEL: usize = 1;
/// The model of a commander.
pub const COMMANDER_MODEL: usize = 2;
/// The model of a projectile.
pub const PROJECTILE_MODEL: usize = 3;

/// The four models in the order of the constants above.
#[must_use]
pub fn marker_models() -> Vec<Model> {
    Vec::new()
}

#[cfg(test)]
mod tests;
