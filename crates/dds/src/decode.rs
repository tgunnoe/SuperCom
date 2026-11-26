//! Pixel decoding: the checked size of a texture and its levels in 8-bit
//! red, green, blue, alpha.

use crate::Error;

/// The largest width or height a texture may have.
pub const MAX_SIDE: u32 = 8192;

/// One decoded mip level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height` pixels of red, green, blue, alpha, rows in stored order.
    pub rgba: Vec<u8>,
}

/// The bytes the decoded chain of the file `bytes` will occupy.
///
/// # Errors
///
/// Not yet written: always an error.
pub fn decoded_len(bytes: &[u8]) -> Result<usize, Error> {
    Err(not_written(bytes))
}

/// Decodes every level of the file `bytes`.
///
/// # Errors
///
/// Not yet written: always an error.
pub fn decode(bytes: &[u8]) -> Result<Vec<Level>, Error> {
    Err(not_written(bytes))
}

fn not_written(bytes: &[u8]) -> Error {
    Error::Malformed {
        reason: format!("the decoder is not written ({} bytes given)", bytes.len()),
    }
}
