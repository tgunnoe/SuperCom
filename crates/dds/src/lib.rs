//! Reader for the headers of `.dds` textures: dimensions, mip count, and
//! pixel-format classification. Pixel data stays opaque: block-compressed
//! data is decoded by the renderer, not here.
//!
//! A `.dds` file is a 128-byte little-endian header (the magic `DDS ` and a
//! 124-byte `DDS_HEADER`) followed by the pixel data. The header layout is
//! the one Microsoft documents at
//! <https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header>:
//! `dwSize` at offset 4, flags at 8, height at 12, width at 16, mip count at
//! 28, the pixel format at 76 (its flags at 80, FOURCC at 84, RGB bit count
//! at 88).
//!
//! This crate is a skeleton: [`Header::parse`] returns
//! [`Error::Unimplemented`], and the test suites under `tests/` are written
//! to fail against it.

use std::fmt;

/// How the pixels of a texture are stored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// Block-compressed, FOURCC `DXT1`: 8 bytes per 4x4 block.
    Dxt1,
    /// Block-compressed, FOURCC `DXT3`: 16 bytes per 4x4 block.
    Dxt3,
    /// Block-compressed, FOURCC `DXT5`: 16 bytes per 4x4 block.
    Dxt5,
    /// No FOURCC: raw pixels of the stated depth, from the pixel format's
    /// RGB bit count.
    Uncompressed {
        /// Bits per pixel.
        bits_per_pixel: u32,
    },
    /// A FOURCC this crate does not classify.
    Other {
        /// The four FOURCC bytes.
        fourcc: [u8; 4],
    },
}

/// The parsed header of a `.dds` file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Header {
    /// Width of the top mip level, in pixels.
    pub width: u32,
    /// Height of the top mip level, in pixels.
    pub height: u32,
    /// Number of mip levels: the header's count when its
    /// `DDSD_MIPMAPCOUNT` flag (0x20000) is set, else 1 (the field is
    /// ignored).
    pub mip_count: u32,
    /// The pixel format.
    pub format: Format,
    /// Number of bytes after the 128-byte header.
    pub data_len: usize,
}

impl Header {
    /// Parses the header of a `.dds` file; `bytes` is the whole file or any
    /// prefix of at least 128 bytes.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] when `bytes` is shorter than the 128-byte header;
    /// [`Error::BadMagic`] when the file does not start with `DDS `;
    /// [`Error::Malformed`] when `dwSize` is not 124. The skeleton always
    /// returns [`Error::Unimplemented`].
    pub fn parse(_bytes: &[u8]) -> Result<Self, Error> {
        Err(Error::Unimplemented)
    }
}

/// Why a header could not be parsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The file does not start with the magic `DDS `.
    BadMagic {
        /// The four bytes the file starts with.
        found: [u8; 4],
    },
    /// The format version is not the one this crate reads. DDS has no
    /// version field, so the parser does not return it today; the variant
    /// keeps the error shape the three loader crates share.
    UnsupportedVersion {
        /// The version number found.
        found: u32,
    },
    /// The buffer ends before a field the header places.
    Truncated {
        /// The length the buffer would need to hold what was being read.
        needed: usize,
        /// The length of the buffer.
        len: usize,
    },
    /// The header contradicts itself.
    Malformed {
        /// What disagreed, for the reader of the message.
        reason: String,
    },
    /// Reserved for the skeleton: the operation is not written yet. Nothing
    /// returns it once the parser exists.
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
