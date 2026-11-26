//! Pixel decoding: the checked size of a texture and its levels in 8-bit
//! red, green, blue, alpha, in integer arithmetic only.
//!
//! The block layouts are the published ones; the choices the pages leave
//! open (the rounding of interpolated values, the form of a DXT3 or DXT5
//! colour block) are fixed by the specification clauses FMT-061 to FMT-073
//! and the record behind them.

use crate::{Error, Format, Header};

/// The largest width or height a texture may have.
pub const MAX_SIDE: u32 = 8192;

const HEADER_LEN: usize = 128;
const MASKS_AT: usize = 92;
const CAPS2_AT: usize = 112;
const CUBE_MAP: u32 = 0x200;
const VOLUME: u32 = 0x20_0000;

/// One decoded mip level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height` pixels of red, green, blue, alpha, rows in stored
    /// order with the first stored row first.
    pub rgba: Vec<u8>,
}

/// How the pixels of a checked texture are stored.
#[derive(Clone, Copy)]
enum Storage {
    Dxt1,
    Dxt3,
    Dxt5,
    /// The byte of the pixel word each channel reads; alpha is `None` for a
    /// zero alpha mask.
    Raw {
        channels: [usize; 3],
        alpha: Option<usize>,
    },
}

/// A texture that passed every check of FMT-071.
struct Plan<'a> {
    width: u32,
    height: u32,
    mips: u32,
    storage: Storage,
    data: &'a [u8],
    decoded: usize,
}

fn word(bytes: &[u8], at: usize) -> u32 {
    bytes
        .get(at..at + 4)
        .and_then(|b| b.first_chunk::<4>())
        .map_or(0, |b| u32::from_le_bytes(*b))
}

/// The byte a mask of eight aligned bits selects.
fn mask_byte(mask: u32) -> Option<usize> {
    (0..4).find(|&k| mask == 0xFF_u32 << (8 * k))
}

fn level_size(width: u32, height: u32, level: u32) -> (u32, u32) {
    ((width >> level).max(1), (height >> level).max(1))
}

fn block_bytes(storage: Storage) -> u64 {
    match storage {
        Storage::Dxt1 => 8,
        _ => 16,
    }
}

/// Checks the header against the data, in the order of FMT-071, before
/// anything is allocated or indexed.
fn plan(bytes: &[u8]) -> Result<Plan<'_>, Error> {
    let header = Header::parse(bytes)?;
    let (width, height) = (header.width, header.height);
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(Error::BadDimensions { width, height });
    }
    let most = width.max(height).ilog2() + 1;
    if header.mip_count == 0 || header.mip_count > most {
        return Err(Error::BadMipCount {
            mip_count: header.mip_count,
            most,
        });
    }
    let caps2 = word(bytes, CAPS2_AT);
    if caps2 & CUBE_MAP != 0 {
        return Err(Error::UnsupportedKind { kind: "cube map" });
    }
    if caps2 & VOLUME != 0 {
        return Err(Error::UnsupportedKind {
            kind: "volume texture",
        });
    }
    let storage = match header.format {
        Format::Dxt1 => Storage::Dxt1,
        Format::Dxt3 => Storage::Dxt3,
        Format::Dxt5 => Storage::Dxt5,
        Format::Uncompressed { bits_per_pixel: 32 } => raw_storage(bytes)?,
        found => return Err(Error::UnsupportedFormat { found }),
    };
    let (mut chain, mut decoded) = (0_u64, 0_u64);
    for level in 0..header.mip_count {
        let (w, h) = level_size(width, height, level);
        let (w, h) = (u64::from(w), u64::from(h));
        decoded += w * h * 4;
        chain += match storage {
            Storage::Raw { .. } => w * h * 4,
            _ => w.div_ceil(4) * h.div_ceil(4) * block_bytes(storage),
        };
    }
    let data = bytes.get(HEADER_LEN..).unwrap_or_default();
    let have = u64::try_from(data.len()).unwrap_or(u64::MAX);
    if chain > have {
        return Err(Error::Truncated {
            needed: usize::try_from(chain)
                .unwrap_or(usize::MAX)
                .saturating_add(HEADER_LEN),
            len: bytes.len(),
        });
    }
    Ok(Plan {
        width,
        height,
        mips: header.mip_count,
        storage,
        data,
        decoded: usize::try_from(decoded).map_err(|_| Error::BadDimensions { width, height })?,
    })
}

fn raw_storage(bytes: &[u8]) -> Result<Storage, Error> {
    let masks = [
        word(bytes, MASKS_AT),
        word(bytes, MASKS_AT + 4),
        word(bytes, MASKS_AT + 8),
        word(bytes, MASKS_AT + 12),
    ];
    let mut picked = Vec::with_capacity(4);
    for (channel, mask) in ['R', 'G', 'B', 'A'].into_iter().zip(masks) {
        if channel == 'A' && mask == 0 {
            continue;
        }
        let byte = mask_byte(mask).ok_or(Error::BadChannelMask { channel, mask })?;
        picked.push((byte, mask));
    }
    for (i, &(byte, mask)) in picked.iter().enumerate() {
        if picked.iter().take(i).any(|&(other, _)| other == byte) {
            return Err(Error::OverlappingChannelMasks { mask });
        }
    }
    let bytes_of: Vec<usize> = picked.iter().map(|&(byte, _)| byte).collect();
    match bytes_of[..] {
        [r, g, b] => Ok(Storage::Raw {
            channels: [r, g, b],
            alpha: None,
        }),
        [r, g, b, a] => Ok(Storage::Raw {
            channels: [r, g, b],
            alpha: Some(a),
        }),
        _ => Err(Error::BadChannelMask {
            channel: 'R',
            mask: 0,
        }),
    }
}

/// The bytes the decoded chain of the file `bytes` will occupy: the sum of
/// `width * height * 4` over its levels. Every check of the decoder is made
/// first, so a size returned is one the data bears out, and it is never more
/// than 8 bytes for each byte of data.
///
/// # Errors
///
/// Any refusal of [`decode`], with the same value.
pub fn decoded_len(bytes: &[u8]) -> Result<usize, Error> {
    plan(bytes).map(|plan| plan.decoded)
}

/// Decodes every level of the file `bytes`, top first.
///
/// # Errors
///
/// [`Error::Truncated`] when the header or the chain is cut short;
/// [`Error::BadDimensions`], [`Error::BadMipCount`], [`Error::UnsupportedKind`],
/// [`Error::UnsupportedFormat`], [`Error::BadChannelMask`], and
/// [`Error::OverlappingChannelMasks`] for what the header says; and the
/// errors of [`Header::parse`].
pub fn decode(bytes: &[u8]) -> Result<Vec<Level>, Error> {
    let plan = plan(bytes)?;
    let mut levels = Vec::with_capacity(usize::try_from(plan.mips).unwrap_or(0));
    let mut rest = plan.data;
    for level in 0..plan.mips {
        let (width, height) = level_size(plan.width, plan.height, level);
        let (used, rgba) = match plan.storage {
            Storage::Raw { channels, alpha } => decode_raw(rest, width, height, channels, alpha),
            storage => decode_blocks(rest, width, height, storage),
        };
        rest = rest.get(used..).unwrap_or_default();
        levels.push(Level {
            width,
            height,
            rgba,
        });
    }
    Ok(levels)
}

fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// A value of at most 255 as a byte.
fn byte(value: u32) -> u8 {
    u8::try_from(value).unwrap_or(u8::MAX)
}

fn decode_raw(
    data: &[u8],
    width: u32,
    height: u32,
    channels: [usize; 3],
    alpha: Option<usize>,
) -> (usize, Vec<u8>) {
    let pixels = to_usize(width) * to_usize(height);
    let src = data.get(..pixels * 4).unwrap_or_default();
    let mut out = Vec::with_capacity(pixels * 4);
    for px in src.chunks_exact(4) {
        let at = |k: usize| px.get(k).copied().unwrap_or(0);
        out.extend_from_slice(&[
            at(channels[0]),
            at(channels[1]),
            at(channels[2]),
            alpha.map_or(255, at),
        ]);
    }
    (pixels * 4, out)
}

/// An 8-bit channel from five bits.
fn expand5(v: u32) -> u32 {
    (v << 3) | (v >> 2)
}

/// An 8-bit channel from six bits.
fn expand6(v: u32) -> u32 {
    (v << 2) | (v >> 4)
}

/// `n / d` rounded to the nearest integer, a tie rounding up.
fn nearest(n: u32, d: u32) -> u32 {
    (n + d / 2) / d
}

fn endpoint(c: u16) -> [u32; 3] {
    let c = u32::from(c);
    [expand5(c >> 11), expand6((c >> 5) & 63), expand5(c & 31)]
}

/// The four colours of a block's code table. `three_colour` is whether
/// `color0 <= color1` selects the three-colour form (DXT1 only).
fn palette(c0: u16, c1: u16, three_colour: bool) -> [[u8; 4]; 4] {
    let (e0, e1) = (endpoint(c0), endpoint(c1));
    let mix = |wa: u32, wb: u32, d: u32| {
        let ch = |i: usize| byte(nearest(wa * e0[i] + wb * e1[i], d));
        [ch(0), ch(1), ch(2), 255]
    };
    let solid = |e: [u32; 3]| [byte(e[0]), byte(e[1]), byte(e[2]), 255];
    if three_colour && c0 <= c1 {
        [solid(e0), solid(e1), mix(1, 1, 2), [0, 0, 0, 0]]
    } else {
        [solid(e0), solid(e1), mix(2, 1, 3), mix(1, 2, 3)]
    }
}

fn le64(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .rev()
        .fold(0_u64, |acc, &b| (acc << 8) | u64::from(b))
}

/// The eight alpha values of a DXT5 block.
fn alpha_table(a0: u8, a1: u8) -> [u8; 8] {
    let (x0, x1) = (u32::from(a0), u32::from(a1));
    let mut table = [a0, a1, 0, 0, 0, 0, 0, 0];
    if a0 > a1 {
        for k in 1..=6_u32 {
            table[to_usize(k) + 1] = byte(nearest((7 - k) * x0 + k * x1, 7));
        }
    } else {
        for k in 1..=4_u32 {
            table[to_usize(k) + 1] = byte(nearest((5 - k) * x0 + k * x1, 5));
        }
        table[6] = 0;
        table[7] = 255;
    }
    table
}

/// Decodes one level; returns the bytes of the chain it used and its pixels.
fn decode_blocks(data: &[u8], width: u32, height: u32, storage: Storage) -> (usize, Vec<u8>) {
    let size = to_usize(u32::try_from(block_bytes(storage)).unwrap_or(16));
    let (w, h) = (to_usize(width), to_usize(height));
    let (blocks_x, blocks_y) = (w.div_ceil(4), h.div_ceil(4));
    let mut out = vec![0_u8; w * h * 4];
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let at = (by * blocks_x + bx) * size;
            let block = data.get(at..at + size).unwrap_or_default();
            let texels = block_texels(block, storage);
            for y in 0..4.min(h - by * 4) {
                for x in 0..4.min(w - bx * 4) {
                    let dst = ((by * 4 + y) * w + bx * 4 + x) * 4;
                    if let (Some(slot), Some(px)) =
                        (out.get_mut(dst..dst + 4), texels.get(4 * y + x))
                    {
                        slot.copy_from_slice(px);
                    }
                }
            }
        }
    }
    (blocks_x * blocks_y * size, out)
}

/// The sixteen pixels of one block, in texel order `4 * y + x`.
fn block_texels(block: &[u8], storage: Storage) -> [[u8; 4]; 16] {
    let mut texels = [[0_u8; 4]; 16];
    let (alpha_part, colour_part) = block.split_at(block.len().saturating_sub(8));
    let c0 = u16::from_le_bytes([
        colour_part.first().copied().unwrap_or(0),
        colour_part.get(1).copied().unwrap_or(0),
    ]);
    let c1 = u16::from_le_bytes([
        colour_part.get(2).copied().unwrap_or(0),
        colour_part.get(3).copied().unwrap_or(0),
    ]);
    let codes = le64(colour_part.get(4..8).unwrap_or_default());
    let table = palette(c0, c1, matches!(storage, Storage::Dxt1));
    let alpha_word = le64(alpha_part.get(2..).unwrap_or_default());
    let dxt3_word = le64(alpha_part);
    let alphas = alpha_table(
        alpha_part.first().copied().unwrap_or(0),
        alpha_part.get(1).copied().unwrap_or(0),
    );
    for (i, texel) in texels.iter_mut().enumerate() {
        let code = to_usize(u32::try_from((codes >> (2 * i)) & 3).unwrap_or(0));
        let mut px = table.get(code).copied().unwrap_or_default();
        match storage {
            Storage::Dxt3 => {
                px[3] = byte(u32::try_from((dxt3_word >> (4 * i)) & 15).unwrap_or(0) * 17);
            }
            Storage::Dxt5 => {
                let index = to_usize(u32::try_from((alpha_word >> (3 * i)) & 7).unwrap_or(0));
                px[3] = alphas.get(index).copied().unwrap_or(0);
            }
            Storage::Dxt1 | Storage::Raw { .. } => {}
        }
        *texel = px;
    }
    texels
}
