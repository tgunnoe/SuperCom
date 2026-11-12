//! Hermetic behaviour of `supercom-render` over maps built in code. No game
//! is needed and nothing is drawn: every test builds a synthetic heightmap
//! and checks the grid, the water plane, the camera, or the shading against
//! a computation written here independently of the crate.

mod common;

use common::{
    clip, close, cross, dot, finite_matrix, flat_map, frame, grid_map, length, ok, pan_to,
    peak_mesh, projects_inside, set_distance, sub, to_usize, unit, with_water,
};
use supercom_render::{
    Camera, Error, LIGHT_DIRECTION, TerrainMesh, WaterPlane, ZoomSpeed, grid_vertex_count,
    normalize_height, shade,
};

/// The relative tolerance the camera clauses state for inside-the-view tests.
const TOL: f32 = 1e-4;

fn triangles(mesh: &TerrainMesh) -> Vec<[[f32; 3]; 3]> {
    mesh.indices
        .chunks_exact(3)
        .map(|t| {
            [
                mesh.positions[to_usize(t[0])],
                mesh.positions[to_usize(t[1])],
                mesh.positions[to_usize(t[2])],
            ]
        })
        .collect()
}

fn triangle_normal(t: &[[f32; 3]; 3]) -> [f32; 3] {
    cross(sub(t[1], t[0]), sub(t[2], t[0]))
}

// REN-001
#[test]
fn render_elevation_is_sample_times_scale() {
    let samples: [i16; 9] = [0, 1, 2, 3, -4, 5, 100, 200, 32767];
    let map = grid_map(2, 2, 2.0, 2.0, 0.5, |i, j| {
        samples[usize::from(j) * 3 + usize::from(i)]
    });
    let mesh = ok(TerrainMesh::build(&map));
    assert_eq!(mesh.positions.len(), 9);
    for (position, sample) in mesh.positions.iter().zip(samples) {
        assert!(
            close(position[1], f32::from(sample) * 0.5, 1e-3),
            "{position:?} for sample {sample}"
        );
    }
    assert!(close(mesh.lowest, -2.0, 1e-6));
    assert!(close(mesh.highest, 16383.5, 1e-3));
}

// REN-002
#[test]
fn render_grid_has_one_vertex_per_sample() {
    let map = grid_map(4, 2, 64.0, 40.0, 1.0, |i, j| {
        i16::from(i) + 10 * i16::from(j)
    });
    let mesh = ok(TerrainMesh::build(&map));
    assert_eq!(mesh.positions.len(), 15);
    assert_eq!(mesh.normals.len(), 15);
    for j in 0..=2_u16 {
        for i in 0..=4_u16 {
            let v = mesh.positions[usize::from(j) * 5 + usize::from(i)];
            assert!(
                close(v[0], f32::from(i) * 16.0, 1e-4),
                "x of ({i},{j}): {v:?}"
            );
            assert!(
                close(v[2], f32::from(j) * 20.0, 1e-4),
                "z of ({i},{j}): {v:?}"
            );
        }
    }
    let last = mesh.positions[14];
    assert!(close(last[0], 64.0, 1e-4) && close(last[2], 40.0, 1e-4));
}

// REN-003
#[test]
fn render_indices_form_two_upward_triangles_per_cell() {
    let mesh = ok(TerrainMesh::build(&flat_map(3, 2, 96.0, 64.0, 5)));
    assert_eq!(mesh.indices.len(), 36);
    assert!(mesh.indices.iter().all(|&i| to_usize(i) < 12));
    let tris = triangles(&mesh);
    assert_eq!(tris.len(), 12);
    let mut area = 0.0;
    for t in &tris {
        let n = triangle_normal(t);
        assert!(n[1] > 0.0, "triangle {t:?} does not face up");
        area += n[1] / 2.0;
    }
    assert!(close(area, 96.0 * 64.0, 1e-1), "triangles cover {area}");
}

// REN-004
#[test]
fn render_malformed_maps_are_errors() {
    assert!(TerrainMesh::build(&flat_map(2, 2, 8.0, 8.0, 0)).is_ok());

    let mut short = flat_map(2, 2, 8.0, 8.0, 0);
    short.heightmap.samples.pop();
    assert!(matches!(
        TerrainMesh::build(&short),
        Err(Error::SampleCount {
            expected: 9,
            found: 8
        })
    ));

    let mut zero = flat_map(2, 2, 8.0, 8.0, 0);
    zero.heightmap.width = 0;
    zero.heightmap.samples.truncate(3);
    assert!(matches!(
        TerrainMesh::build(&zero),
        Err(Error::ZeroCells {
            columns: 0,
            rows: 2
        })
    ));

    for scale in [f32::NAN, f32::INFINITY] {
        let mut bad = flat_map(2, 2, 8.0, 8.0, 0);
        bad.heightmap.scale = scale;
        assert!(matches!(
            TerrainMesh::build(&bad),
            Err(Error::NonFiniteScale { .. })
        ));
    }

    let mut huge = flat_map(2, 2, 8.0, 8.0, 100);
    huge.heightmap.scale = f32::MAX;
    assert!(matches!(
        TerrainMesh::build(&huge),
        Err(Error::NonFiniteElevation)
    ));

    for (w, h) in [
        (0.0, 8.0),
        (8.0, -1.0),
        (f32::NAN, 8.0),
        (8.0, f32::INFINITY),
    ] {
        let bad = flat_map(2, 2, w, h, 0);
        assert!(
            matches!(TerrainMesh::build(&bad), Err(Error::BadExtent { .. })),
            "extent {w} by {h}"
        );
    }
}

// REN-005
#[test]
fn render_vertex_count_bound_is_32_bits() {
    assert_eq!(grid_vertex_count(4096, 4096), Ok(16_785_409));
    assert_eq!(grid_vertex_count(2, 3), Ok(12));
    assert_eq!(grid_vertex_count(65_534, 65_535), Ok(4_294_901_760));
    for (columns, rows) in [(65_535, 65_535), (u32::MAX, 1), (u32::MAX, u32::MAX)] {
        assert_eq!(
            grid_vertex_count(columns, rows),
            Err(Error::TooManyVertices { columns, rows })
        );
    }
}

// REN-006
#[test]
fn render_normals_are_unit_length_and_up_on_flat_terrain() {
    let flat = ok(TerrainMesh::build(&flat_map(4, 3, 40.0, 30.0, 7)));
    assert_eq!(flat.normals.len(), 20);
    for n in &flat.normals {
        assert!(close(n[0], 0.0, 1e-6) && close(n[1], 1.0, 1e-6) && close(n[2], 0.0, 1e-6));
    }

    // A plane rising 0.5 per world unit along x: central differences are
    // exact on it, so every normal is (-0.5, 1, 0) normalized, edges included.
    let ramp = ok(TerrainMesh::build(&grid_map(
        4,
        3,
        40.0,
        30.0,
        1.0,
        |i, _| i16::from(i) * 5,
    )));
    assert_eq!(ramp.normals.len(), 20);
    let want = unit([-0.5, 1.0, 0.0]);
    for n in &ramp.normals {
        assert!(close(length(*n), 1.0, 1e-5));
        assert!(
            close(n[0], want[0], 1e-5) && close(n[1], want[1], 1e-5) && close(n[2], want[2], 1e-5),
            "{n:?} against {want:?}"
        );
    }

    let rough = peak_mesh();
    assert_eq!(rough.normals.len(), 49);
    assert!(
        rough
            .normals
            .iter()
            .all(|n| close(length(*n), 1.0, 1e-5) && n[1] > 0.0)
    );
}

// REN-007
#[test]
fn render_water_plane_matches_stored_elevation() {
    let dry = flat_map(2, 2, 256.0, 128.0, 0);
    assert!(WaterPlane::from_map(&dry).is_none());

    let wet = with_water(dry, 17.5);
    let plane = WaterPlane::from_map(&wet);
    assert!(plane.is_some(), "a map with water has a plane");
    let corners = plane.map(|p| p.corners).unwrap_or_default();
    assert_eq!(corners.len(), 4);
    assert!(corners.iter().all(|c| close(c[1], 17.5, 1e-6)));
    let xs: Vec<f32> = corners.iter().map(|c| c[0]).collect();
    let zs: Vec<f32> = corners.iter().map(|c| c[2]).collect();
    assert!(close(
        xs.iter().copied().fold(f32::MAX, f32::min),
        0.0,
        1e-6
    ));
    assert!(close(
        xs.iter().copied().fold(f32::MIN, f32::max),
        256.0,
        1e-6
    ));
    assert!(close(
        zs.iter().copied().fold(f32::MAX, f32::min),
        0.0,
        1e-6
    ));
    assert!(close(
        zs.iter().copied().fold(f32::MIN, f32::max),
        128.0,
        1e-6
    ));
    for t in [
        [corners[0], corners[1], corners[2]],
        [corners[0], corners[2], corners[3]],
    ] {
        assert!(triangle_normal(&t)[1] > 0.0, "{t:?} faces down");
    }
}

// REN-008
#[test]
fn render_matrices_are_column_major_with_depth_zero_to_one() {
    let mut camera = ok(Camera::new(512.0, 256.0, 0.0, 200.0, 1.5));
    set_distance(&mut camera, 200.0);
    let m = camera.view_projection();
    let f = frame(&camera, 1.5);
    let (near, far) = camera.clip_planes();
    assert!(near > 0.0 && far > near);
    let at = |depth: f32| {
        let p = [
            f.eye[0] + f.forward[0] * depth,
            f.eye[1] + f.forward[1] * depth,
            f.eye[2] + f.forward[2] * depth,
        ];
        clip(&m, p)
    };
    let (n, fr) = (at(near), at(far));
    assert!(close(n[2] / n[3], 0.0, 1e-4), "near depth {}", n[2] / n[3]);
    assert!(
        close(fr[2] / fr[3], 1.0, 1e-4),
        "far depth {}",
        fr[2] / fr[3]
    );
    // The last row of a perspective projection copies the view depth to w.
    assert!(close(n[3], near, 1e-2) && close(fr[3], far, far * 1e-4));
}

// REN-009
#[test]
fn render_target_projects_to_centre_on_highest_plane() {
    let mut camera = ok(Camera::new(512.0, 256.0, 3.0, 90.0, 16.0 / 9.0));
    for (distance, x, z) in [(1e9, 0.0, 0.0), (300.0, 100.0, -50.0), (0.0, 1e9, 1e9)] {
        set_distance(&mut camera, distance);
        pan_to(&mut camera, x, z);
        let t = camera.target();
        assert!(
            close(t[1], 90.0, 1e-4),
            "target {t:?} off the highest plane"
        );
        assert!(camera.eye()[1] >= t[1]);
        assert!(close(length(sub(camera.eye(), t)), camera.distance(), 1e-2));
        let c = clip(&camera.view_projection(), t);
        assert!(c[3] > 0.0);
        assert!(
            close(c[0] / c[3], 0.0, 1e-4) && close(c[1] / c[3], 0.0, 1e-4),
            "{c:?}"
        );
    }
}

// REN-010
#[test]
fn render_zoom_range_far_end_is_the_strategic_view() {
    let tan = std::f32::consts::FRAC_PI_8.tan();
    for aspect in [1.0_f32, 16.0 / 9.0, 0.5] {
        let camera = ok(Camera::new(512.0, 256.0, 0.0, 50.0, aspect));
        let (near, far) = camera.zoom_range();
        assert!(close(near, 40.0, 1e-6), "near end {near}");
        let want = (256.0 / (tan * aspect)).max(128.0 / tan);
        assert!(close(far, want, want * 1e-4), "far end {far}, want {want}");
        assert!(
            close(camera.distance(), far, far * 1e-5),
            "starts at the strategic view"
        );
    }

    let mut camera = ok(Camera::new(512.0, 256.0, 0.0, 50.0, 1.0));
    let (near, far) = camera.zoom_range();
    let range = far - near;
    camera.zoom(-1.0, ZoomSpeed::Normal);
    assert!(close(camera.distance(), far - 0.02 * range, range * 1e-4));
    camera.zoom(-1.0, ZoomSpeed::Fast);
    assert!(close(camera.distance(), far - 0.10 * range, range * 1e-4));
    camera.zoom(0.5, ZoomSpeed::Fast);
    assert!(close(camera.distance(), far - 0.06 * range, range * 1e-4));
    camera.zoom(1000.0, ZoomSpeed::Normal);
    assert!(
        close(camera.distance(), far, far * 1e-5),
        "clamped to the far end"
    );
    camera.zoom(-1000.0, ZoomSpeed::Fast);
    assert!(
        close(camera.distance(), near, 1e-4),
        "clamped to the near end"
    );
}

// REN-011
#[test]
fn render_zoom_out_from_a_corner_shows_the_whole_extent() {
    for aspect in [1.0_f32, 16.0 / 9.0] {
        let mesh = peak_mesh();
        let mut camera = ok(Camera::for_mesh(&mesh, aspect));
        let (near, far) = camera.zoom_range();
        let corners = [
            [0.0, 200.0, 0.0],
            [512.0, 200.0, 0.0],
            [0.0, 200.0, 256.0],
            [512.0, 200.0, 256.0],
        ];
        for corner in corners {
            camera.zoom(-1000.0, ZoomSpeed::Fast);
            pan_to(&mut camera, corner[0], corner[2]);
            let t = camera.target();
            assert!(close(camera.distance(), near, 1e-4));
            assert!(
                close(t[0], corner[0], 1e-3) && close(t[2], corner[2], 1e-3),
                "{t:?}"
            );
            for _ in 0..400 {
                if camera.distance() >= far {
                    break;
                }
                camera.zoom(1.0, ZoomSpeed::Normal);
            }
            assert!(close(camera.distance(), far, far * 1e-5));
            let m = camera.view_projection();
            for c in corners {
                assert!(
                    projects_inside(&m, c, TOL),
                    "corner {c:?} off screen at aspect {aspect}"
                );
            }
        }

        camera.go_strategic();
        let (_, far) = camera.zoom_range();
        let (near, _) = camera.zoom_range();
        let steps = -0.01 * far / (0.02 * (far - near));
        camera.zoom(steps, ZoomSpeed::Normal);
        assert!(close(camera.distance(), 0.99 * far, far * 1e-4));
        let t = camera.target();
        assert!(
            close(t[0], 256.0, 1e-2) && close(t[2], 128.0, 1e-2),
            "centre target {t:?}"
        );
        let m = camera.view_projection();
        assert!(
            corners.iter().any(|c| !projects_inside(&m, *c, TOL)),
            "a corner must leave the screen one percent inside the far end"
        );
    }
}

// REN-012
#[test]
fn render_pitch_tilts_continuously_with_north_up_east_right() {
    let mut camera = ok(Camera::new(512.0, 512.0, 0.0, 100.0, 1.0));
    let (near, far) = camera.zoom_range();
    let pitch_of = |c: &Camera| (c.eye()[1] - c.target()[1]).atan2(c.eye()[2] - c.target()[2]);
    assert!(
        close(pitch_of(&camera), std::f32::consts::FRAC_PI_2, 1e-4),
        "straight down at far"
    );
    camera.zoom(-1000.0, ZoomSpeed::Normal);
    assert!(
        close(pitch_of(&camera), 35.0_f32.to_radians(), 1e-4),
        "35 degrees at near"
    );

    let mut previous = pitch_of(&camera);
    let mut seen = 0;
    for k in 0..=100_u16 {
        set_distance(&mut camera, near + (far - near) * f32::from(k) / 100.0);
        let pitch = pitch_of(&camera);
        assert!(
            close(pitch, camera.pitch(), 1e-4),
            "pitch accessor {} against {pitch}",
            camera.pitch()
        );
        assert!(
            pitch >= previous - 1e-5,
            "pitch must not fall as the view zooms out"
        );
        assert!(
            pitch - previous < 0.02,
            "pitch jumps from {previous} to {pitch}"
        );
        assert!(finite_matrix(&camera.view_projection()));
        previous = pitch;
        seen += 1;
    }
    assert_eq!(seen, 101);

    for d in [near, far] {
        set_distance(&mut camera, d);
        pan_to(&mut camera, 256.0, 256.0);
        let m = camera.view_projection();
        let t = camera.target();
        let here = clip(&m, t);
        let east = clip(&m, [t[0] + 10.0, t[1], t[2]]);
        let north = clip(&m, [t[0], t[1], t[2] - 10.0]);
        assert!(
            east[0] / east[3] > here[0] / here[3],
            "east is right at distance {d}"
        );
        assert!(
            north[1] / north[3] > here[1] / here[3],
            "north is up at distance {d}"
        );
        assert!(dot(sub(camera.eye(), t), [0.0, 1.0, 0.0]) > 0.0);
    }
}

// REN-013
#[test]
fn render_pan_clamps_target_inside_narrowed_extent() {
    let mut camera = ok(Camera::new(512.0, 256.0, 0.0, 100.0, 1.0));
    let (near, far) = camera.zoom_range();
    pan_to(&mut camera, 1e6, 1e6);
    assert!(
        close(camera.target()[0], 256.0, 1e-3),
        "at the far end the target is the centre"
    );
    assert!(close(camera.target()[2], 128.0, 1e-3));

    camera.zoom(-1000.0, ZoomSpeed::Normal);
    pan_to(&mut camera, 256.0, 128.0);
    camera.pan(30.0, -20.0);
    let t = camera.target();
    assert!(
        close(t[0], 286.0, 1e-3) && close(t[2], 108.0, 1e-3),
        "plain move {t:?}"
    );
    camera.pan(1e6, 1e6);
    let t = camera.target();
    assert!(
        close(t[0], 512.0, 1e-3) && close(t[2], 256.0, 1e-3),
        "clamped to the corner {t:?}"
    );
    camera.pan(-1e7, -1e7);
    let t = camera.target();
    assert!(
        close(t[0], 0.0, 1e-3) && close(t[2], 0.0, 1e-3),
        "clamped to the origin {t:?}"
    );

    let mut checked = 0;
    for k in 0..=20_u16 {
        set_distance(&mut camera, near + (far - near) * f32::from(k) / 20.0);
        for (x, z) in [(0.0, 0.0), (512.0, 256.0), (512.0, 0.0), (0.0, 256.0)] {
            pan_to(&mut camera, x, z);
            let narrowing = (far - camera.distance()) / (far - near);
            let t = camera.target();
            assert!(
                (t[0] - 256.0).abs() <= 256.0 * narrowing + 1e-2,
                "x {t:?} at {narrowing}"
            );
            assert!(
                (t[2] - 128.0).abs() <= 128.0 * narrowing + 1e-2,
                "z {t:?} at {narrowing}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 84);
}

// REN-014
#[test]
fn render_clip_planes_bound_every_visible_vertex() {
    let mesh = peak_mesh();
    assert!(
        mesh.highest - mesh.lowest > 40.0,
        "the relief must exceed the near end"
    );
    let peak = mesh.positions[to_usize(7 + 4)];
    assert!(close(peak[1], 200.0, 1e-4));
    let targets = [
        [256.0, 128.0],
        [0.0, 0.0],
        [512.0, 0.0],
        [0.0, 256.0],
        [512.0, 256.0],
        [peak[0], peak[2]],
    ];
    for aspect in [1.0_f32, 16.0 / 9.0] {
        let probe = ok(Camera::for_mesh(&mesh, aspect));
        let (near, far) = probe.zoom_range();
        for fraction in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
            for target in targets {
                let mut camera = ok(Camera::for_mesh(&mesh, aspect));
                set_distance(&mut camera, near + (far - near) * fraction);
                pan_to(&mut camera, target[0], target[1]);
                let f = frame(&camera, aspect);
                let (clip_near, clip_far) = camera.clip_planes();
                assert!(
                    f.eye[1] > mesh.highest,
                    "eye {:?} not above the peak",
                    f.eye
                );
                let mut checked = 0;
                for p in &mesh.positions {
                    if let Some(depth) = f.depth_inside(*p, TOL) {
                        checked += 1;
                        assert!(
                            depth >= clip_near && depth <= clip_far,
                            "vertex {p:?} at depth {depth} outside {clip_near}..{clip_far} \
                             (fraction {fraction}, target {target:?}, aspect {aspect})"
                        );
                    }
                }
                assert!(
                    checked > 0,
                    "no vertex inside the view (fraction {fraction}, target {target:?})"
                );
            }
        }
    }
}

// REN-015
#[test]
fn render_height_normalization_and_shading() {
    assert!(close(normalize_height(5.0, 5.0, 25.0), 0.0, 1e-6));
    assert!(close(normalize_height(25.0, 5.0, 25.0), 1.0, 1e-6));
    assert!(close(normalize_height(10.0, 5.0, 25.0), 0.25, 1e-6));
    assert!(close(normalize_height(-3.0, -3.0, -3.0), 0.0, 0.0));
    assert!(close(normalize_height(7.5, 7.5, 7.5), 0.0, 0.0));

    assert!(close(LIGHT_DIRECTION[0], -0.36, 1e-6));
    assert!(close(LIGHT_DIRECTION[1], 0.8, 1e-6));
    assert!(close(LIGHT_DIRECTION[2], -0.48, 1e-6));
    assert!(close(length(LIGHT_DIRECTION), 1.0, 1e-6));

    let luminance = |c: [f32; 3]| c[0] + c[1] + c[2];
    let tilt = |angle: f32| {
        // Rotate the light direction away from itself toward +x/+z along a
        // great circle: the dot with the light falls as the angle grows.
        let side = unit(cross(LIGHT_DIRECTION, [0.0, 1.0, 0.0]));
        let (s, c) = angle.sin_cos();
        [
            LIGHT_DIRECTION[0] * c + side[0] * s,
            LIGHT_DIRECTION[1] * c + side[1] * s,
            LIGHT_DIRECTION[2] * c + side[2] * s,
        ]
    };
    for height in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        let mut previous = f32::MAX;
        let mut strict = 0;
        for k in 0..=12_u16 {
            let n = tilt(f32::from(k) * std::f32::consts::PI / 12.0);
            assert!(close(length(n), 1.0, 1e-5));
            let colour = shade(height, n);
            assert!(colour.iter().all(|c| (0.0..=1.0).contains(c)), "{colour:?}");
            let l = luminance(colour);
            assert!(l <= previous + 1e-6, "darker toward the light at step {k}");
            if l < previous - 1e-4 {
                strict += 1;
            }
            previous = l;
        }
        assert!(
            strict >= 3,
            "shading barely depends on the normal at height {height}"
        );
    }
    let lit = luminance(shade(0.5, LIGHT_DIRECTION));
    let unlit = luminance(shade(0.5, [0.0, -1.0, 0.0]));
    assert!(lit > unlit + 0.1, "lit {lit} against unlit {unlit}");
    let (valley, summit) = (shade(0.0, [0.0, 1.0, 0.0]), shade(1.0, [0.0, 1.0, 0.0]));
    assert!(
        luminance(summit) > luminance(valley) + 0.1,
        "colour follows height: {valley:?} against {summit:?}"
    );
}

// REN-016
#[test]
fn render_aspect_change_recomputes_range_and_reclamps() {
    let tan = std::f32::consts::FRAC_PI_8.tan();
    let mut camera = ok(Camera::new(512.0, 256.0, 0.0, 50.0, 1.0));
    let far_at = |aspect: f32| (256.0 / (tan * aspect)).max(128.0 / tan);
    let (_, far_square) = camera.zoom_range();
    assert!(close(far_square, far_at(1.0), far_square * 1e-4));

    assert!(camera.set_aspect(4.0).is_ok());
    let (near, far_wide) = camera.zoom_range();
    assert!(
        far_wide < far_square,
        "a wider screen needs a shorter far end"
    );
    assert!(close(far_wide, far_at(4.0), far_wide * 1e-4));
    assert!(close(near, 40.0, 1e-6));
    assert!(
        close(camera.distance(), far_wide, far_wide * 1e-5),
        "distance re-clamped to the new far end"
    );

    camera.zoom(-1000.0, ZoomSpeed::Normal);
    pan_to(&mut camera, 512.0, 256.0);
    assert!(camera.set_aspect(0.5).is_ok());
    let (_, far_tall) = camera.zoom_range();
    assert!(close(far_tall, far_at(0.5), far_tall * 1e-4));
    assert!(
        close(camera.distance(), near, 1e-4),
        "an in-range distance stays"
    );
    assert!(close(camera.target()[0], 512.0, 1e-3));

    camera.zoom(1000.0, ZoomSpeed::Normal);
    assert!(
        close(camera.target()[0], 256.0, 1e-2),
        "far end re-centres the target"
    );
    let before = camera;
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(
            matches!(camera.set_aspect(bad), Err(Error::BadAspect { .. })),
            "aspect {bad}"
        );
        assert_eq!(camera, before, "a refused aspect changes nothing");
    }
}

// REN-017
#[test]
fn render_camera_rejects_bad_inputs() {
    assert!(Camera::new(512.0, 256.0, 0.0, 50.0, 1.5).is_ok());
    let flat = ok(Camera::new(512.0, 256.0, 7.0, 7.0, 1.5));
    assert!(finite_matrix(&flat.view_projection()));
    assert!(close(flat.target()[1], 7.0, 1e-6));

    let nan = f32::NAN;
    let inf = f32::INFINITY;
    for (w, h, lo, hi, a) in [
        (0.0, 256.0, 0.0, 1.0, 1.0),
        (-5.0, 256.0, 0.0, 1.0, 1.0),
        (nan, 256.0, 0.0, 1.0, 1.0),
        (512.0, inf, 0.0, 1.0, 1.0),
        (512.0, 0.0, 0.0, 1.0, 1.0),
        (512.0, 256.0, nan, 1.0, 1.0),
        (512.0, 256.0, 0.0, inf, 1.0),
        (512.0, 256.0, 5.0, 4.0, 1.0),
        (512.0, 256.0, 0.0, 1.0, 0.0),
        (512.0, 256.0, 0.0, 1.0, nan),
        (512.0, 256.0, 0.0, 1.0, -2.0),
        (10.0, 10.0, 0.0, 1.0, 1.0),
    ] {
        assert!(
            Camera::new(w, h, lo, hi, a).is_err(),
            "accepted extent {w} by {h}, elevations {lo}..{hi}, aspect {a}"
        );
    }
    assert!(matches!(
        Camera::new(512.0, 256.0, 5.0, 4.0, 1.0),
        Err(Error::BadElevationRange { .. })
    ));
}
