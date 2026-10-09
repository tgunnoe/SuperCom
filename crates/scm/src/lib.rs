//! Loader for `.scm` meshes and `.sca` animations: the binary formats of the
//! installed game's models.
//!
//! Both formats are little-endian. A mesh (`MODL`, version 5) is a 48-byte
//! header followed by tables of bones, vertices, and indices placed by file
//! offsets; an animation (`ANIM`, version 5) is a 36-byte header followed by
//! bone names, bone links, a root pose, and fixed-size frames.
//!
//! This crate is a skeleton: every parse function returns
//! [`Error::Unimplemented`], and the test suites under `tests/` are written
//! to fail against it. Layouts follow the pinned reading of the Blender
//! importer at <https://github.com/Exotic-Retard/SupCom_Import_Export_Blender>
//! (commit d5cd48b6f0c3, `supcom-importer.py` lines 208, 281, 328, 412, 496,
//! 502, 625); that importer carries no licence, so it is cited and not copied.

use std::fmt;

/// One bone of a mesh skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    /// The bone's name: the NUL-terminated ASCII string at the record's name
    /// offset.
    pub name: String,
    /// Index of the parent bone, or `None` for a root (stored as -1).
    pub parent: Option<usize>,
    /// Rest position of the bone.
    pub position: [f32; 3],
    /// Rest rotation of the bone, as a quaternion in file order.
    pub rotation: [f32; 4],
    /// The rest-pose inverse matrix as stored: row-major, not transposed.
    pub rest_pose_inverse: [[f32; 4]; 4],
}

/// One vertex of a mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Vertex {
    /// Position.
    pub position: [f32; 3],
    /// Tangent.
    pub tangent: [f32; 3],
    /// Normal.
    pub normal: [f32; 3],
    /// Binormal.
    pub binormal: [f32; 3],
    /// First texture coordinate set.
    pub uv0: [f32; 2],
    /// Second texture coordinate set.
    pub uv1: [f32; 2],
    /// The four bone indices that skin the vertex.
    pub bone_indices: [u8; 4],
}

/// A parsed `.scm` mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// The skeleton: `total_bone_count` records from the header.
    pub bones: Vec<Bone>,
    /// The vertex table.
    pub vertices: Vec<Vertex>,
    /// The index table: `index_count` values, three to a triangle.
    pub indices: Vec<u16>,
}

impl Mesh {
    /// Parses the bytes of a `.scm` file.
    ///
    /// # Errors
    ///
    /// [`Error::BadMagic`] when the file does not start with `MODL`;
    /// [`Error::UnsupportedVersion`] for any version but 5;
    /// [`Error::Truncated`] when the buffer is shorter than the 48-byte
    /// header or than a table the header places; [`Error::Malformed`] for a
    /// table the header describes inconsistently. The skeleton always returns
    /// [`Error::Unimplemented`].
    pub fn parse(_bytes: &[u8]) -> Result<Self, Error> {
        Err(Error::Unimplemented)
    }
}

/// A bone's position and rotation at one instant of an animation.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    /// Position.
    pub position: [f32; 3],
    /// Rotation, as a quaternion in file order.
    pub rotation: [f32; 4],
}

/// One key frame of an animation: a key time and one pose per bone.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The key time, in seconds.
    pub time: f32,
    /// One pose per bone, in the order of [`Animation::bone_names`].
    pub poses: Vec<Pose>,
}

/// A parsed `.sca` animation.
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    /// The frame count from the header; equals `frames.len()`.
    pub frame_count: u32,
    /// The duration in seconds, from the header.
    pub duration: f32,
    /// The animated bones' names, in file order.
    pub bone_names: Vec<String>,
    /// The bone links as stored: one `i32` per bone, the parent index.
    pub bone_links: Vec<i32>,
    /// The single pose that precedes the frames. It is the animation's root
    /// pose and is not a frame.
    pub root: Pose,
    /// The key frames, in file order.
    pub frames: Vec<Frame>,
}

impl Animation {
    /// Parses the bytes of a `.sca` file.
    ///
    /// # Errors
    ///
    /// [`Error::BadMagic`] when the file does not start with `ANIM`;
    /// [`Error::UnsupportedVersion`] for any version but 5;
    /// [`Error::Truncated`] when the buffer is shorter than the 36-byte
    /// header or than the data the header places; [`Error::Malformed`] when
    /// `frame_size` is not `8 + 28 * bone_count`. The skeleton always returns
    /// [`Error::Unimplemented`].
    pub fn parse(_bytes: &[u8]) -> Result<Self, Error> {
        Err(Error::Unimplemented)
    }
}

/// Why a mesh or animation could not be parsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The file does not start with the expected magic.
    BadMagic {
        /// The four bytes the file starts with.
        found: [u8; 4],
    },
    /// The format version is not the one this crate reads.
    UnsupportedVersion {
        /// The version number the header carries.
        found: u32,
    },
    /// The buffer ends before a field or table the header places.
    Truncated {
        /// The length the buffer would need to hold what was being read.
        needed: usize,
        /// The length of the buffer.
        len: usize,
    },
    /// The header and the data it describes disagree.
    Malformed {
        /// What disagreed, for the reader of the message.
        reason: String,
    },
    /// Reserved for the skeleton: the operation is not written yet. Nothing
    /// returns it once the parsers exist.
    Unimplemented,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic { found } => write!(f, "bad magic {found:?}"),
            Self::UnsupportedVersion { found } => write!(f, "unsupported version {found}"),
            Self::Truncated { needed, len } => {
                write!(f, "truncated: needs {needed} bytes, has {len}")
            }
            Self::Malformed { reason } => write!(f, "malformed: {reason}"),
            Self::Unimplemented => f.write_str("not implemented"),
        }
    }
}

impl std::error::Error for Error {}
