//! The four marker models: flat shapes in the plane y = 0.

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
    vec![
        // A square, north-west, south-west, north-east, south-east.
        flat(
            &[[-0.5, -0.5], [-0.5, 0.5], [0.5, -0.5], [0.5, 0.5]],
            &[0, 1, 2, 2, 1, 3],
        ),
        // A triangle whose tip is on +x.
        flat(&[[0.5, 0.0], [-0.4, -0.25], [-0.4, 0.25]], &[0, 1, 2]),
        // A dart: the tip on +x, two tail corners, and a notch between them.
        flat(
            &[[0.5, 0.0], [-0.3, -0.35], [-0.1, 0.0], [-0.3, 0.35]],
            &[0, 1, 2, 0, 2, 3],
        ),
        // A narrow rhombus.
        flat(
            &[[0.5, 0.0], [0.0, -0.25], [-0.5, 0.0], [0.0, 0.25]],
            &[0, 1, 2, 0, 2, 3],
        ),
    ]
}

/// A model in the plane y = 0 from `(x, z)` corners and triangle indices,
/// every normal up.
fn flat(corners: &[[f32; 2]], indices: &[u32]) -> Model {
    Model {
        positions: corners.iter().map(|[x, z]| [*x, 0.0, *z]).collect(),
        normals: corners.iter().map(|_| [0.0, 1.0, 0.0]).collect(),
        indices: indices.to_vec(),
    }
}

#[cfg(test)]
mod tests;
