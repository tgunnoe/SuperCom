//! The crate's error type.

use std::fmt;

/// Why a mesh or a camera could not be built, or why the GPU side could not
/// draw. GPU-side causes carry wgpu's own text as a `String`, because wgpu's
/// error types are neither `Clone` nor `PartialEq`.
#[derive(Clone, Debug, PartialEq)]
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
    /// An elevation difference over finite elevations overflowed, so a
    /// vertex normal is not finite.
    NonFiniteNormal,
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
    /// No adapter exists among the backends searched.
    NoAdapter {
        /// What wgpu reported.
        reason: String,
    },
    /// The adapter refused the device request.
    DeviceRequest {
        /// What wgpu reported.
        reason: String,
    },
    /// The surface could not be created or is unusable with the adapter.
    Surface {
        /// What went wrong.
        reason: String,
    },
    /// A buffer the grid needs would exceed the device's maximum buffer
    /// size.
    GridTooLarge {
        /// Cells along x.
        columns: u32,
        /// Cells along z.
        rows: u32,
        /// Which buffer: `"vertex"` or `"index"`.
        buffer: &'static str,
        /// The size in bytes that buffer would need.
        bytes: u64,
        /// The device's maximum buffer size in bytes.
        limit: u64,
    },
    /// The mesh's buffers do not match its cell counts.
    InconsistentMesh,
    /// A frame was drawn before a mesh was uploaded.
    NothingUploaded,
    /// The render target has a zero dimension.
    ZeroTarget {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// wgpu reported a validation or out-of-memory error while the
    /// renderer created resources or recorded a frame.
    Gpu {
        /// What the renderer was doing.
        during: &'static str,
        /// What wgpu reported.
        reason: String,
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
            Self::NonFiniteNormal => {
                write!(
                    f,
                    "a vertex normal is not finite although every elevation is"
                )
            }
            Self::NoAdapter { reason } => write!(f, "no graphics adapter is available: {reason}"),
            Self::DeviceRequest { reason } => {
                write!(f, "the adapter refused the device request: {reason}")
            }
            Self::Surface { reason } => write!(f, "the surface is unusable: {reason}"),
            Self::GridTooLarge {
                columns,
                rows,
                buffer,
                bytes,
                limit,
            } => write!(
                f,
                "a grid of {columns} by {rows} cells needs a {buffer} buffer of {bytes} bytes, over the device's {limit}"
            ),
            Self::InconsistentMesh => {
                write!(
                    f,
                    "the mesh's vertex and index counts do not match its cell counts"
                )
            }
            Self::NothingUploaded => write!(f, "a frame was drawn before any mesh was uploaded"),
            Self::ZeroTarget { width, height } => {
                write!(
                    f,
                    "the render target is {width} by {height} pixels; both must be positive"
                )
            }
            Self::Gpu { during, reason } => {
                write!(f, "wgpu reported an error during {during}: {reason}")
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
