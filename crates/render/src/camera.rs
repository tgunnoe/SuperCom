//! The camera: a top-down strategy view that pans over the map and zooms
//! between a tilted close view and a straight-down strategic one.
//!
//! The target always lies on the horizontal plane at the map's highest
//! elevation, and the eye sits south of and above it, so north is up on the
//! screen and east is to the right. The view is built from the pitch
//! directly, so it is finite even looking straight down.

use crate::{Error, TerrainMesh};
use std::f32::consts::FRAC_PI_2;

/// Vertical field of view in radians (45 degrees).
pub const FOV_Y_RADIANS: f32 = std::f32::consts::FRAC_PI_4;
/// Near end of the zoom range: the closest eye-to-target distance, in world
/// units.
pub const NEAR_DISTANCE: f32 = 40.0;
/// Pitch at the near end of the zoom range in radians (35 degrees); the
/// pitch is the angle of the view direction below the horizontal.
pub const NEAR_PITCH_RADIANS: f32 = 0.610_865_24;
/// Fraction of the zoom range one normal-speed step moves the distance.
pub const NORMAL_ZOOM_STEP: f32 = 0.02;
/// Fraction of the zoom range one fast-speed step moves the distance.
pub const FAST_ZOOM_STEP: f32 = 0.08;

/// Safety factor on the near clip plane against the smallest depth a
/// visible vertex can have.
const NEAR_CLIP_SHARE: f32 = 0.5;
/// Safety factor on the far clip plane over the largest distance a visible
/// vertex can have.
const FAR_CLIP_MARGIN: f32 = 1.1;

/// How fast a zoom step moves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomSpeed {
    /// [`NORMAL_ZOOM_STEP`] of the zoom range per step.
    Normal,
    /// [`FAST_ZOOM_STEP`] of the zoom range per step.
    Fast,
}

/// A top-down strategy camera over one map. See the module documentation
/// for its geometry; docs/specs/render.md fixes its numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    width: f32,
    height: f32,
    lowest: f32,
    highest: f32,
    aspect: f32,
    far: f32,
    distance: f32,
    x: f32,
    z: f32,
}

/// The distance at which an extent fits the view looking straight down with
/// the target at its centre.
fn strategic_distance(width: f32, height: f32, aspect: f32) -> Result<f32, Error> {
    let tan_half = (FOV_Y_RADIANS / 2.0).tan();
    let (across, down) = (width / 2.0 / tan_half, height / 2.0 / tan_half);
    if !(across.is_finite() && down.is_finite()) {
        return Err(Error::BadExtent { width, height });
    }
    let far = (across / aspect).max(down);
    if !far.is_finite() {
        return Err(Error::BadAspect { aspect });
    }
    if far <= NEAR_DISTANCE {
        return Err(Error::ExtentTooSmall { width, height });
    }
    Ok(far)
}

fn positive_finite(v: f32) -> bool {
    v.is_finite() && v > 0.0
}

/// Product of two column-major matrices, `a` times `b`.
fn multiply(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for (c, column) in out.iter_mut().enumerate() {
        for (r, cell) in column.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

impl Camera {
    /// A camera over a map of the given extent and elevation range, at the
    /// strategic view: the target at the centre, the distance at the far
    /// end of the zoom range.
    ///
    /// `width` and `height` are the extent along x and z in world units,
    /// `lowest` and `highest` the lowest and highest vertex elevations in
    /// world units, and `aspect` the screen's width over its height.
    ///
    /// # Errors
    /// [`Error::BadExtent`] for an extent that is not finite and positive or
    /// so large that its strategic distance is not finite,
    /// [`Error::BadAspect`] for an aspect ratio that is not finite and
    /// positive, [`Error::BadElevationRange`] for an elevation that is not
    /// finite or a highest below the lowest, and [`Error::ExtentTooSmall`]
    /// for an extent whose strategic view is no farther than
    /// [`NEAR_DISTANCE`].
    pub fn new(
        width: f32,
        height: f32,
        lowest: f32,
        highest: f32,
        aspect: f32,
    ) -> Result<Self, Error> {
        if !(positive_finite(width) && positive_finite(height)) {
            return Err(Error::BadExtent { width, height });
        }
        if !positive_finite(aspect) {
            return Err(Error::BadAspect { aspect });
        }
        if !(lowest.is_finite() && highest.is_finite() && (highest - lowest).is_finite())
            || highest < lowest
        {
            return Err(Error::BadElevationRange { lowest, highest });
        }
        let far = strategic_distance(width, height, aspect)?;
        Ok(Self {
            width,
            height,
            lowest,
            highest,
            aspect,
            far,
            distance: far,
            x: width / 2.0,
            z: height / 2.0,
        })
    }

    /// A camera over `mesh`'s extent and elevation range.
    ///
    /// # Errors
    /// As [`Camera::new`].
    pub fn for_mesh(mesh: &TerrainMesh, aspect: f32) -> Result<Self, Error> {
        Self::new(mesh.width, mesh.height, mesh.lowest, mesh.highest, aspect)
    }

    /// Zooms by `steps` steps of `speed`, each moving the distance by a
    /// fixed fraction of the zoom range; a positive number zooms toward the
    /// strategic end, a fractional one by that fraction of a step. The
    /// distance clamps to the range and the target re-clamps to the region
    /// the new distance allows. A step count that is not finite is ignored.
    pub fn zoom(&mut self, steps: f32, speed: ZoomSpeed) {
        if !steps.is_finite() {
            return;
        }
        let fraction = match speed {
            ZoomSpeed::Normal => NORMAL_ZOOM_STEP,
            ZoomSpeed::Fast => FAST_ZOOM_STEP,
        };
        self.distance = (self.distance + steps * fraction * (self.far - NEAR_DISTANCE))
            .clamp(NEAR_DISTANCE, self.far);
        self.clamp_target();
    }

    /// Moves the target by `dx` along x and `dz` along z, in world units,
    /// then clamps it to the region the current distance allows. An offset
    /// that is not finite is ignored.
    pub fn pan(&mut self, dx: f32, dz: f32) {
        if !(dx.is_finite() && dz.is_finite()) {
            return;
        }
        self.x += dx;
        self.z += dz;
        self.clamp_target();
    }

    /// Sets the aspect ratio (screen width over height), recomputing the
    /// zoom range and re-clamping the distance and the target.
    ///
    /// # Errors
    /// [`Error::BadAspect`] for an aspect ratio that is not finite and
    /// positive, or [`Error::ExtentTooSmall`] when the new range is empty;
    /// the camera is unchanged.
    pub fn set_aspect(&mut self, aspect: f32) -> Result<(), Error> {
        if !positive_finite(aspect) {
            return Err(Error::BadAspect { aspect });
        }
        self.far = strategic_distance(self.width, self.height, aspect)?;
        self.aspect = aspect;
        self.distance = self.distance.clamp(NEAR_DISTANCE, self.far);
        self.clamp_target();
        Ok(())
    }

    /// Goes to the strategic view: the target at the map's centre and the
    /// distance at the far end of the zoom range.
    pub fn go_strategic(&mut self) {
        self.distance = self.far;
        self.x = self.width / 2.0;
        self.z = self.height / 2.0;
    }

    /// The target `[x, y, z]` in world units; y is the highest vertex
    /// elevation.
    #[must_use]
    pub fn target(&self) -> [f32; 3] {
        [self.x, self.highest, self.z]
    }

    /// The eye `[x, y, z]` in world units: above the target's plane and
    /// south of the target.
    #[must_use]
    pub fn eye(&self) -> [f32; 3] {
        let (sin, cos) = self.pitch().sin_cos();
        [
            self.x,
            self.highest + self.distance * sin,
            self.z + self.distance * cos,
        ]
    }

    /// The eye-to-target distance in world units, within
    /// [`Camera::zoom_range`].
    #[must_use]
    pub fn distance(&self) -> f32 {
        self.distance
    }

    /// The zoom range `(near, far)` of distances, in world units. The far
    /// end is the strategic view for the current aspect ratio.
    #[must_use]
    pub fn zoom_range(&self) -> (f32, f32) {
        (NEAR_DISTANCE, self.far)
    }

    /// The pitch in radians, the angle of the view direction below the
    /// horizontal: a quarter turn at the far end, [`NEAR_PITCH_RADIANS`] at
    /// the near end, linear in the distance between.
    #[must_use]
    pub fn pitch(&self) -> f32 {
        FRAC_PI_2 - (FRAC_PI_2 - NEAR_PITCH_RADIANS) * self.nearness()
    }

    /// The clip planes `(near, far)` as distances along the view direction,
    /// in world units, for the current distance: every vertex of the map
    /// inside the view has a depth between them.
    #[must_use]
    pub fn clip_planes(&self) -> (f32, f32) {
        let (sin, cos) = self.pitch().sin_cos();
        let tan_half = (FOV_Y_RADIANS / 2.0).tan();
        // The eye stands `distance * sin` above the highest vertex, and a
        // ray in the view that reaches that plane is steepest at the lower
        // edge of the view, so no visible vertex is nearer than this.
        let nearest = self.distance * sin / (sin + tan_half * cos);
        let diagonal = self.width.hypot(self.height);
        let farthest = (diagonal + self.distance * cos)
            .hypot(self.distance * sin + (self.highest - self.lowest));
        (NEAR_CLIP_SHARE * nearest, FAR_CLIP_MARGIN * farthest)
    }

    /// The combined view-projection matrix.
    ///
    /// `m[c][r]` is row `r` of column `c`; a world point maps to clip space
    /// as the matrix times the column vector `(x, y, z, 1)`; and the array's
    /// memory layout equals WGSL's `mat4x4<f32>`. Clip space has y up and
    /// depth 0 at the near clip plane and 1 at the far clip plane.
    #[must_use]
    pub fn view_projection(&self) -> [[f32; 4]; 4] {
        let (near, far) = self.clip_planes();
        let (sin, cos) = self.pitch().sin_cos();
        let eye = self.eye();
        let tan_half = (FOV_Y_RADIANS / 2.0).tan();

        // The camera's basis: right is +x, forward looks north and down by
        // the pitch, and up completes a right-handed frame.
        let view = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, cos, sin, 0.0],
            [0.0, -sin, cos, 0.0],
            [
                -eye[0],
                -(cos * eye[1] - sin * eye[2]),
                -(sin * eye[1] + cos * eye[2]),
                1.0,
            ],
        ];
        let depth_scale = far / (near - far);
        let projection = [
            [1.0 / (tan_half * self.aspect), 0.0, 0.0, 0.0],
            [0.0, 1.0 / tan_half, 0.0, 0.0],
            [0.0, 0.0, depth_scale, -1.0],
            [0.0, 0.0, near * depth_scale, 0.0],
        ];
        multiply(&projection, &view)
    }

    /// How far the distance is toward the near end: 0 at the far end, 1 at
    /// the near end.
    fn nearness(&self) -> f32 {
        ((self.far - self.distance) / (self.far - NEAR_DISTANCE)).clamp(0.0, 1.0)
    }

    /// Clamps the target to the extent scaled about its centre by
    /// [`Camera::nearness`]: the whole extent at the near end, the centre
    /// alone at the far end.
    fn clamp_target(&mut self) {
        let reach = self.nearness();
        let (half_x, half_z) = (self.width / 2.0, self.height / 2.0);
        self.x = self
            .x
            .clamp(half_x - half_x * reach, half_x + half_x * reach);
        self.z = self
            .z
            .clamp(half_z - half_z * reach, half_z + half_z * reach);
    }
}
