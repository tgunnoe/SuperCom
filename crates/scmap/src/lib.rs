//! Loader for the binary `.scmap` map file: the terrain heightmap, the
//! embedded preview image, and the terrain texture layers (strata).
//!
//! A `.scmap` is little-endian. It opens with a 30-byte fixed header
//! (signature `0x1a70614d`, major version 2, two constant words, the map's
//! width and height as `f32`, a zero `i32`, a zero `i16`), then the preview
//! image (an `i32` byte length, then that many bytes of a whole `.dds`
//! file), an `i32` inner version (56 or 60), the heightmap, and a long run
//! of sections (terrain shader paths, cube maps, lighting, water and its
//! wave generators, minimap colours) that this crate walks to reach the
//! terrain strata. Decals, props, and the sections after the strata are not
//! read.
//!
//! The layout follows the MIT-licensed reference reader
//! `SCMapImporter.java` at
//! <https://github.com/FAForever/Neroxis-Map-Generator> (commit 10b130836034,
//! `shared/src/main/java/com/faforever/neroxis/importer/SCMapImporter.java`,
//! lines 58-149): the preview at 78 and 446-453, the heightmap at 82-89, the
//! strata at 126-136. The map file is binary; the `_scenario.lua` and
//! `_save.lua` beside it are not read here.
//!
//! This crate is a skeleton: [`Map::parse`] returns [`Error::Unimplemented`],
//! and the test suites under `tests/` are written to fail against it.

use std::fmt;

/// The terrain heightmap.
#[derive(Clone, Debug, PartialEq)]
pub struct Heightmap {
    /// Width in cells; the sample grid is one larger in each direction.
    pub width: u32,
    /// Height in cells.
    pub height: u32,
    /// World units per height-sample step, as stored.
    pub scale: f32,
    /// `(width + 1) * (height + 1)` signed 16-bit samples, row by row.
    pub samples: Vec<i16>,
}

/// One terrain texture layer: a path and a tiling scale.
#[derive(Clone, Debug, PartialEq)]
pub struct Stratum {
    /// The texture's path as the map stores it: it may start with `/`, use
    /// `\`, and mix case, and is empty for an unused layer.
    pub texture: String,
    /// The tiling scale stored after the path.
    pub scale: f32,
}

/// A parsed `.scmap` map.
#[derive(Clone, Debug, PartialEq)]
pub struct Map {
    /// Map width in world units, from the fixed header.
    pub width: f32,
    /// Map height in world units, from the fixed header.
    pub height: f32,
    /// The inner version: 56 or 60.
    pub version: u32,
    /// The embedded preview image: a whole `.dds` file.
    pub preview: Vec<u8>,
    /// The terrain heightmap.
    pub heightmap: Heightmap,
    /// The ten terrain texture layers in file order, unused ones with an
    /// empty `texture`.
    pub strata: Vec<Stratum>,
}

impl Map {
    /// Parses the bytes of a `.scmap` file.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] when `bytes` is shorter than the 30-byte fixed
    /// header or than a section being read; [`Error::BadSignature`] when the
    /// first word is not `0x1a70614d`; [`Error::UnsupportedVersion`] for an
    /// inner version other than 56 or 60; [`Error::Malformed`] for a negative
    /// length or dimension. The skeleton always returns
    /// [`Error::Unimplemented`].
    pub fn parse(_bytes: &[u8]) -> Result<Self, Error> {
        Err(Error::Unimplemented)
    }
}

/// Why a map could not be parsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The file does not start with the map signature.
    BadSignature {
        /// The first four bytes as a little-endian word.
        found: u32,
    },
    /// The inner version is neither 56 nor 60.
    UnsupportedVersion {
        /// The version number the file carries.
        found: u32,
    },
    /// The buffer ends before a section being read.
    Truncated {
        /// The length the buffer would need to hold what was being read.
        needed: usize,
        /// The length of the buffer.
        len: usize,
    },
    /// A length or dimension is impossible.
    Malformed {
        /// What was impossible, for the reader of the message.
        reason: String,
    },
    /// Reserved for the skeleton: the operation is not written yet. Nothing
    /// returns it once the parser exists.
    Unimplemented,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadSignature { found } => write!(f, "bad signature {found:#010x}"),
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
