//! The crate's error type.

use std::fmt;

/// Why a mesh or a camera could not be built.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    /// The sample count is not `(width + 1) * (height + 1)`.
    SampleCount {
        /// The count the cell counts require.
        expected: u64,
        /// The count the map holds.
        found: usize,
    },
    /// A cell count is zero.
    ZeroCells {
        /// Cells along x.
        columns: u32,
        /// Cells along z.
        rows: u32,
    },
    /// The heightmap scale is NaN or infinite.
    NonFiniteScale {
        /// The scale as stored.
        scale: f32,
    },
    /// A world extent is not finite and positive.
    BadExtent {
        /// Extent along x in world units.
        width: f32,
        /// Extent along z in world units.
        height: f32,
    },
    /// The vertex count does not fit 32-bit indices.
    TooManyVertices {
        /// Cells along x.
        columns: u32,
        /// Cells along z.
        rows: u32,
    },
    /// A sample times the scale is not finite.
    NonFiniteElevation,
    /// The aspect ratio is not finite and positive.
    BadAspect {
        /// The aspect ratio as given.
        aspect: f32,
    },
    /// An elevation is not finite, or the highest is below the lowest.
    BadElevationRange {
        /// The lowest elevation as given.
        lowest: f32,
        /// The highest elevation as given.
        highest: f32,
    },
    /// The extent is so small that its strategic view is nearer than the
    /// near end of the zoom range.
    ExtentTooSmall {
        /// Extent along x in world units.
        width: f32,
        /// Extent along z in world units.
        height: f32,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SampleCount { expected, found } => {
                write!(
                    f,
                    "heightmap holds {found} samples, its cells need {expected}"
                )
            }
            Self::ZeroCells { columns, rows } => {
                write!(
                    f,
                    "heightmap has {columns} by {rows} cells; both must be positive"
                )
            }
            Self::NonFiniteScale { scale } => write!(f, "heightmap scale {scale} is not finite"),
            Self::BadExtent { width, height } => {
                write!(f, "extent {width} by {height} is not finite and positive")
            }
            Self::TooManyVertices { columns, rows } => write!(
                f,
                "a grid of {columns} by {rows} cells has more vertices than 32-bit indices address"
            ),
            Self::NonFiniteElevation => {
                write!(f, "a sample times the scale is not a finite elevation")
            }
            Self::BadAspect { aspect } => {
                write!(f, "aspect ratio {aspect} is not finite and positive")
            }
            Self::BadElevationRange { lowest, highest } => write!(
                f,
                "elevation range {lowest} to {highest} is not finite or is reversed"
            ),
            Self::ExtentTooSmall { width, height } => write!(
                f,
                "extent {width} by {height} is smaller than the camera's near zoom distance allows"
            ),
        }
    }
}

impl std::error::Error for Error {}
