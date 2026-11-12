//! Helpers shared by the hermetic tests: maps built in code, small vector
//! algebra written independently of the crate under test, and the pyramid
//! test the camera clauses use.

use std::fmt::Debug;
use supercom_render::{Camera, TerrainMesh};
use supercom_scmap::{Heightmap, Map, Water};

/// Tangent of half the 45 degree vertical field of view, stated in the
/// camera clauses and computed here without the crate.
pub fn tan_half_fov() -> f32 {
    std::f32::consts::FRAC_PI_8.tan()
}

/// Whether `a` and `b` differ by at most `tol`.
pub fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

#[expect(
    clippy::expect_used,
    reason = "test helper; clippy.toml exempts only #[test] functions"
)]
pub fn ok<T, E: Debug>(result: Result<T, E>) -> T {
    result.expect("the call was given well-formed input")
}

pub fn to_usize(n: u32) -> usize {
    ok(usize::try_from(n))
}

/// A map of `cols` by `rows` cells over a world extent, with `sample(i, j)`
/// at column `i`, row `j`, and no water.
pub fn grid_map(
    cols: u8,
    rows: u8,
    width: f32,
    height: f32,
    scale: f32,
    sample: impl Fn(u8, u8) -> i16,
) -> Map {
    let mut samples = Vec::new();
    for j in 0..=rows {
        for i in 0..=cols {
            samples.push(sample(i, j));
        }
    }
    Map {
        width,
        height,
        version: 60,
        preview: Vec::new(),
        heightmap: Heightmap {
            width: u32::from(cols),
            height: u32::from(rows),
            scale,
            samples,
        },
        strata: Vec::new(),
        water: None,
    }
}

pub fn flat_map(cols: u8, rows: u8, width: f32, height: f32, level: i16) -> Map {
    grid_map(cols, rows, width, height, 1.0, |_, _| level)
}

pub fn with_water(mut map: Map, elevation: f32) -> Map {
    map.water = Some(Water {
        elevation,
        deep: elevation - 2.0,
        abyss: elevation - 4.0,
    });
    map
}

/// A 6 by 6 cell map over 512 by 256 world units with a peak of 200 on the
/// vertex at column 4, row 1 and a floor of 0, so its relief exceeds the
/// camera's near zoom distance; the centre and corners are vertices.
pub fn peak_map() -> Map {
    grid_map(6, 6, 512.0, 256.0, 1.0, |i, j| {
        if (i, j) == (4, 1) {
            200
        } else {
            i16::from((i + j) % 3) * 10
        }
    })
}

pub fn peak_mesh() -> TerrainMesh {
    ok(TerrainMesh::build(&peak_map()))
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

pub fn unit(a: [f32; 3]) -> [f32; 3] {
    let n = length(a);
    [a[0] / n, a[1] / n, a[2] / n]
}

/// The matrix times the column vector `(p, 1)`; `m[c][r]` is row `r` of
/// column `c`.
pub fn clip(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 4] {
    let mut out = [0.0; 4];
    for (r, slot) in out.iter_mut().enumerate() {
        *slot = m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
    }
    out
}

/// The camera's frame, computed from its eye and target alone: forward
/// toward the target, right along +x, and up completing a right-handed
/// frame with the view looking down its own -z.
pub struct Frame {
    pub eye: [f32; 3],
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub aspect: f32,
}

pub fn frame(camera: &Camera, aspect: f32) -> Frame {
    let eye = camera.eye();
    let forward = unit(sub(camera.target(), eye));
    let right = [1.0, 0.0, 0.0];
    let up = cross([-forward[0], -forward[1], -forward[2]], right);
    Frame {
        eye,
        forward,
        right,
        up,
        aspect,
    }
}

impl Frame {
    /// The depth along the view direction of `p` when `p` lies inside the
    /// four side planes of the view pyramid, else `None`.
    pub fn depth_inside(&self, p: [f32; 3], tol: f32) -> Option<f32> {
        let d = sub(p, self.eye);
        let depth = dot(d, self.forward);
        let tv = tan_half_fov();
        let inside = depth > 0.0
            && dot(d, self.right).abs() <= depth * tv * self.aspect * (1.0 + tol)
            && dot(d, self.up).abs() <= depth * tv * (1.0 + tol);
        inside.then_some(depth)
    }
}

/// Whether `p` projects inside the screen (clip x and y no larger than w in
/// magnitude, within `tol`) with positive w.
pub fn projects_inside(m: &[[f32; 4]; 4], p: [f32; 3], tol: f32) -> bool {
    let c = clip(m, p);
    c[3] > 0.0 && c[0].abs() <= c[3] * (1.0 + tol) && c[1].abs() <= c[3] * (1.0 + tol)
}

/// Moves the camera to distance `d` by fractional normal steps.
pub fn set_distance(camera: &mut Camera, d: f32) {
    let (near, far) = camera.zoom_range();
    let steps = (d - camera.distance()) / (0.02 * (far - near));
    camera.zoom(steps, supercom_render::ZoomSpeed::Normal);
}

/// Pans so the target requests `(x, z)`.
pub fn pan_to(camera: &mut Camera, x: f32, z: f32) {
    let t = camera.target();
    camera.pan(x - t[0], z - t[2]);
}

pub fn finite_matrix(m: &[[f32; 4]; 4]) -> bool {
    m.iter().flatten().all(|v| v.is_finite())
}
