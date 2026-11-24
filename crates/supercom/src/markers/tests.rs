use super::*;
use supercom_render::check_model;

/// The y component of (b - a) x (c - a), positive when the triangle is
/// wound counter-clockwise seen from above.
fn winding(origin: [f32; 3], second: [f32; 3], third: [f32; 3]) -> f32 {
    let first_edge = [second[0] - origin[0], second[2] - origin[2]];
    let second_edge = [third[0] - origin[0], third[2] - origin[2]];
    first_edge[1] * second_edge[0] - first_edge[0] * second_edge[1]
}

fn distance(p: [f32; 3]) -> f32 {
    (p[0] * p[0] + p[2] * p[2]).sqrt()
}

/// Four distinct shapes, flat, upward, wound counter-clockwise from above,
/// and the two that point have their tip on +x.
#[test]
fn supercom_marker_models_cases() {
    let models = marker_models();
    assert_eq!(models.len(), 4);
    assert_eq!(
        [
            STATIC_MODEL,
            MOBILE_MODEL,
            COMMANDER_MODEL,
            PROJECTILE_MODEL
        ],
        [0, 1, 2, 3]
    );
    let vertices: Vec<usize> = models.iter().map(|m| m.positions.len()).collect();
    assert_eq!(vertices, [4, 3, 4, 4]);
    let indices: Vec<usize> = models.iter().map(|m| m.indices.len()).collect();
    assert_eq!(indices, [6, 3, 6, 6]);

    for (at, model) in models.iter().enumerate() {
        assert_eq!(check_model(at, model), Ok(()), "model {at}");
        assert!(model.positions.iter().all(|p| p[1] == 0.0), "model {at}");
        assert!(
            model.normals.iter().all(|n| n
                .iter()
                .zip([0.0, 1.0, 0.0])
                .all(|(a, b)| (a - b).abs() <= f32::EPSILON)),
            "model {at}"
        );
        for corner in model.indices.chunks_exact(3) {
            let p = |i: usize| model.positions[corner[i] as usize];
            assert!(winding(p(0), p(1), p(2)) > 0.0, "model {at}, {corner:?}");
        }
    }
    for (a, first) in models.iter().enumerate() {
        for (b, second) in models.iter().enumerate().skip(a + 1) {
            assert_ne!(first, second, "models {a} and {b} are one shape");
        }
    }

    for at in [MOBILE_MODEL, COMMANDER_MODEL] {
        let farthest = models[at]
            .positions
            .iter()
            .copied()
            .fold([0.0_f32; 3], |best, p| {
                if distance(p) > distance(best) {
                    p
                } else {
                    best
                }
            });
        assert_eq!(farthest, [0.5, 0.0, 0.0], "model {at} points along +x");
        let ties = models[at]
            .positions
            .iter()
            .filter(|p| distance(**p) >= 0.5)
            .count();
        assert_eq!(ties, 1, "model {at} has one tip");
    }
    let z_extent = |at: usize| {
        models[at]
            .positions
            .iter()
            .map(|p| p[2].abs())
            .fold(0.0_f32, f32::max)
    };
    assert_eq!(z_extent(STATIC_MODEL), 0.5);
    assert_eq!(z_extent(PROJECTILE_MODEL), 0.25);
}
