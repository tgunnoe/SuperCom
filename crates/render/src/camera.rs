//! The camera.

use crate::{Error, TerrainMesh};

/// Vertical field of view in radians.
pub const FOV_Y_RADIANS: f32 = 0.0;
/// Near end of the zoom range in world units.
pub const NEAR_DISTANCE: f32 = 0.0;
/// Pitch at the near end of the zoom range in radians.
pub const NEAR_PITCH_RADIANS: f32 = 0.0;
/// Fraction of the zoom range one normal step moves the distance.
pub const NORMAL_ZOOM_STEP: f32 = 0.0;
/// Fraction of the zoom range one fast step moves the distance.
pub const FAST_ZOOM_STEP: f32 = 0.0;
/// Relative tolerance of the inside-the-view comparisons.
pub const VIEW_TOLERANCE: f32 = 0.0;

/// How fast a zoom step moves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomSpeed {
    /// [`NORMAL_ZOOM_STEP`] of the range per step.
    Normal,
    /// [`FAST_ZOOM_STEP`] of the range per step.
    Fast,
}

/// A top-down strategy camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    aspect: f32,
}

impl Camera {
    /// A camera at the strategic view.
    ///
    /// # Errors
    /// See [`Error`].
    pub fn new(
        width: f32,
        height: f32,
        lowest: f32,
        highest: f32,
        aspect: f32,
    ) -> Result<Self, Error> {
        let _ = (width, height, lowest, highest);
        Ok(Self { aspect })
    }

    /// A camera for `mesh`.
    ///
    /// # Errors
    /// See [`Camera::new`].
    pub fn for_mesh(mesh: &TerrainMesh, aspect: f32) -> Result<Self, Error> {
        Self::new(mesh.width, mesh.height, mesh.lowest, mesh.highest, aspect)
    }

    /// Zooms by `steps` steps; positive zooms out.
    pub fn zoom(&mut self, steps: f32, speed: ZoomSpeed) {
        let _ = (steps, speed);
    }

    /// Moves the target by `(dx, dz)` world units.
    pub fn pan(&mut self, dx: f32, dz: f32) {
        let _ = (dx, dz);
    }

    /// Sets the aspect ratio.
    ///
    /// # Errors
    /// [`Error::BadAspect`].
    pub fn set_aspect(&mut self, aspect: f32) -> Result<(), Error> {
        self.aspect = aspect;
        Ok(())
    }

    /// Goes to the strategic view.
    pub fn go_strategic(&mut self) {}

    /// The eye position.
    #[must_use]
    pub fn eye(&self) -> [f32; 3] {
        [0.0; 3]
    }

    /// The target position.
    #[must_use]
    pub fn target(&self) -> [f32; 3] {
        [0.0; 3]
    }

    /// The eye-to-target distance.
    #[must_use]
    pub fn distance(&self) -> f32 {
        0.0
    }

    /// The pitch in radians.
    #[must_use]
    pub fn pitch(&self) -> f32 {
        0.0
    }

    /// The zoom range `(near, far)` of distances.
    #[must_use]
    pub fn zoom_range(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// The clip planes `(near, far)` as distances along the view direction.
    #[must_use]
    pub fn clip_planes(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// The combined view-projection matrix.
    #[must_use]
    pub fn view_projection(&self) -> [[f32; 4]; 4] {
        [[0.0; 4]; 4]
    }
}
