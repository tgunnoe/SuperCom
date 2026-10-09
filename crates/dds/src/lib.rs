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
//! [`Header::parse`] reads the header and nothing after it: the number of
//! bytes that follow is reported as [`Header::data_len`], and their layout
//! is the renderer's concern.

use std::fmt;

/// Length of the magic plus the `DDS_HEADER`: the pixel data starts here.
const HEADER_LEN: usize = 128;
/// The file's first four bytes.
const MAGIC: [u8; 4] = *b"DDS ";
/// The only `dwSize` the format allows: the length of `DDS_HEADER`.
const HEADER_SIZE: u32 = 124;

// Byte offsets of the header fields this crate reads; all are `u32` words
// except the FOURCC, which is four bytes. See the module documentation.
const SIZE_AT: usize = 4;
const FLAGS_AT: usize = 8;
const HEIGHT_AT: usize = 12;
const WIDTH_AT: usize = 16;
const MIP_COUNT_AT: usize = 28;
const PIXEL_FLAGS_AT: usize = 80;
const FOURCC_AT: usize = 84;
const RGB_BIT_COUNT_AT: usize = 88;

/// `DDSD_MIPMAPCOUNT`: the header's mip-count word is meaningful.
const MIPMAPCOUNT: u32 = 0x2_0000;
/// `DDPF_FOURCC`: the pixel format is named by its FOURCC.
const PIXEL_FOURCC: u32 = 0x4;

/// The little-endian word at offset `AT` of the header. The bound is checked
/// when the crate compiles, so the read cannot fail at run time.
fn word<const AT: usize>(head: &[u8; HEADER_LEN]) -> u32 {
    const { assert!(AT + 4 <= HEADER_LEN) };
    u32::from_le_bytes([head[AT], head[AT + 1], head[AT + 2], head[AT + 3]])
}

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
    /// [`Error::Malformed`] when `dwSize` is not 124.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let Some(head) = bytes.first_chunk::<HEADER_LEN>() else {
            return Err(Error::Truncated {
                needed: HEADER_LEN,
                len: bytes.len(),
            });
        };
        let magic = [head[0], head[1], head[2], head[3]];
        if magic != MAGIC {
            return Err(Error::BadMagic { found: magic });
        }
        let size = word::<SIZE_AT>(head);
        if size != HEADER_SIZE {
            return Err(Error::Malformed {
                reason: format!("dwSize is {size}, not {HEADER_SIZE}"),
            });
        }
        let mip_count = if word::<FLAGS_AT>(head) & MIPMAPCOUNT != 0 {
            word::<MIP_COUNT_AT>(head)
        } else {
            1
        };
        let format = if word::<PIXEL_FLAGS_AT>(head) & PIXEL_FOURCC != 0 {
            let fourcc = [
                head[FOURCC_AT],
                head[FOURCC_AT + 1],
                head[FOURCC_AT + 2],
                head[FOURCC_AT + 3],
            ];
            match &fourcc {
                b"DXT1" => Format::Dxt1,
                b"DXT3" => Format::Dxt3,
                b"DXT5" => Format::Dxt5,
                _ => Format::Other { fourcc },
            }
        } else {
            Format::Uncompressed {
                bits_per_pixel: word::<RGB_BIT_COUNT_AT>(head),
            }
        };
        Ok(Self {
            width: word::<WIDTH_AT>(head),
            height: word::<HEIGHT_AT>(head),
            mip_count,
            format,
            data_len: bytes.len() - HEADER_LEN,
        })
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
        }
    }
}

impl std::error::Error for Error {}
