//! Hermetic behaviour of the pixel decoder of `supercom-dds` over textures
//! built in code. No game is needed and no file is committed: every texture
//! is a 128-byte header assembled from literals followed by blocks or pixels
//! that are invented numbers. Each expected pixel below is worked out by
//! hand from the clause it pins (FMT-064 to FMT-073), not computed by the
//! crate.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_dds::{Error, Format, Level, decode, decoded_len};

const HEADER_LEN: usize = 128;
const MIPMAPCOUNT: u32 = 0x2_0000;

/// A synthetic texture: the header words the decoder reads, then `data`.
struct Tex {
    width: u32,
    height: u32,
    /// `None` leaves the mip-count flag clear.
    mips: Option<u32>,
    /// `None` is an uncompressed pixel format.
    fourcc: Option<[u8; 4]>,
    bits: u32,
    masks: [u32; 4],
    caps2: u32,
    data: Vec<u8>,
}

/// The usual masks of a 32-bit texture: a word of blue, green, red, alpha.
const USUAL_MASKS: [u32; 4] = [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000];

impl Tex {
    fn block(fourcc: [u8; 4], width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            mips: None,
            fourcc: Some(fourcc),
            bits: 0,
            masks: [0; 4],
            caps2: 0,
            data,
        }
    }

    fn dxt1(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self::block(*b"DXT1", width, height, data)
    }

    fn dxt3(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self::block(*b"DXT3", width, height, data)
    }

    fn dxt5(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self::block(*b"DXT5", width, height, data)
    }

    fn raw32(width: u32, height: u32, masks: [u32; 4], data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            mips: None,
            fourcc: None,
            bits: 32,
            masks,
            caps2: 0,
            data,
        }
    }

    fn with_mips(self, mips: u32) -> Self {
        Self {
            mips: Some(mips),
            ..self
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let flags = 0x1 | 0x2 | 0x4 | 0x1000 | self.mips.map_or(0, |_| MIPMAPCOUNT);
        let mut out = b"DDS ".to_vec();
        for word in [
            124,
            flags,
            self.height,
            self.width,
            0,
            0,
            self.mips.unwrap_or(0),
        ] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.resize(76, 0);
        out.extend_from_slice(&32u32.to_le_bytes());
        if let Some(fourcc) = self.fourcc {
            out.extend_from_slice(&0x4u32.to_le_bytes());
            out.extend_from_slice(&fourcc);
        } else {
            out.extend_from_slice(&0x41u32.to_le_bytes());
            out.extend_from_slice(&[0; 4]);
        }
        out.extend_from_slice(&self.bits.to_le_bytes());
        for mask in self.masks {
            out.extend_from_slice(&mask.to_le_bytes());
        }
        out.resize(112, 0);
        out.extend_from_slice(&self.caps2.to_le_bytes());
        out.resize(HEADER_LEN, 0);
        out.extend_from_slice(&self.data);
        out
    }
}

/// A DXT1 colour block: two endpoints and the four code bytes (one a row).
fn colour(c0: u16, c1: u16, rows: [u8; 4]) -> Vec<u8> {
    let mut out = c0.to_le_bytes().to_vec();
    out.extend_from_slice(&c1.to_le_bytes());
    out.extend_from_slice(&rows);
    out
}

/// A DXT3 block: eight bytes of alpha, then a colour block.
fn dxt3_block(alpha: [u8; 8], colour: &[u8]) -> Vec<u8> {
    let mut out = alpha.to_vec();
    out.extend_from_slice(colour);
    out
}

/// A DXT5 block: the two alpha bytes, six bytes of codes, a colour block.
fn dxt5_block(a0: u8, a1: u8, codes: [u8; 6], colour: &[u8]) -> Vec<u8> {
    let mut out = vec![a0, a1];
    out.extend_from_slice(&codes);
    out.extend_from_slice(colour);
    out
}

/// Pixels of red, green, blue, alpha as the flat bytes of a level.
fn flat(pixels: &[[u8; 4]]) -> Vec<u8> {
    pixels.concat()
}

fn decoded(tex: &Tex) -> Vec<Level> {
    let result = decode(&tex.bytes());
    assert!(
        result.is_ok(),
        "decode must succeed, got {:?}",
        result.err()
    );
    result.expect("decoded: asserted above")
}

fn one_level(tex: &Tex) -> Level {
    let mut levels = decoded(tex);
    assert_eq!(levels.len(), 1, "one level");
    levels.remove(0)
}

fn refused(tex: &Tex) -> Error {
    let bytes = tex.bytes();
    let result = decode(&bytes);
    assert!(result.is_err(), "decode must refuse, got {result:?}");
    let size = decoded_len(&bytes);
    assert!(size.is_err(), "decoded_len must refuse, got {size:?}");
    assert_eq!(size.as_ref().err(), result.as_ref().err());
    result.expect_err("an error: asserted above")
}

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];
/// Two thirds red and one third blue; one third red and two thirds blue.
const TWO_THIRDS: [u8; 4] = [170, 0, 85, 255];
const ONE_THIRD: [u8; 4] = [85, 0, 170, 255];
/// Codes 0 1 2 3, then 3 2 1 0, then all 0, then all 3.
const GRID: [u8; 4] = [0xE4, 0x1B, 0x00, 0xFF];

/// FMT-066: `color0 > color1` gives four colours, the interpolants being
/// (2 * 255 + 0) / 3 = 170 and 255 / 3 = 85 of each endpoint's channels.
#[test]
fn decode_dxt1_four_colour_block_reads_its_sixteen_codes() {
    let level = one_level(&Tex::dxt1(4, 4, colour(0xF800, 0x001F, GRID)));
    assert_eq!((level.width, level.height), (4, 4));
    let expected = flat(&[
        RED, BLUE, TWO_THIRDS, ONE_THIRD, //
        ONE_THIRD, TWO_THIRDS, BLUE, RED, //
        RED, RED, RED, RED, //
        ONE_THIRD, ONE_THIRD, ONE_THIRD, ONE_THIRD,
    ]);
    assert_eq!(level.rgba, expected);
}

/// FMT-066: `color0 <= color1` gives three colours and code 3 is black with
/// alpha 0; the midpoint of 0 and 255 is (0 + 255 + 1) / 2 = 128.
#[test]
fn decode_dxt1_three_colour_block_has_midpoint_and_transparent_black() {
    let level = one_level(&Tex::dxt1(4, 4, colour(0x001F, 0xF800, GRID)));
    let mid = [128, 0, 128, 255];
    let expected = flat(&[
        BLUE, RED, mid, CLEAR, //
        CLEAR, mid, RED, BLUE, //
        BLUE, BLUE, BLUE, BLUE, //
        CLEAR, CLEAR, CLEAR, CLEAR,
    ]);
    assert_eq!(level.rgba, expected);
}

/// FMT-066: equal endpoints are not `color0 > color1`, so they take the
/// three-colour form: code 2 is their midpoint and code 3 is clear.
#[test]
fn decode_dxt1_equal_endpoints_use_the_three_colour_form() {
    let level = one_level(&Tex::dxt1(4, 4, colour(0x07E0, 0x07E0, [0xE4, 0xFF, 0, 0])));
    let green = [0, 255, 0, 255];
    let expected = flat(&[
        green, green, green, CLEAR, //
        CLEAR, CLEAR, CLEAR, CLEAR, //
        green, green, green, green, //
        green, green, green, green,
    ]);
    assert_eq!(level.rgba, expected);
}

/// FMT-065: 5 bits 16 expand to (16 << 3) | (16 >> 2) = 132, 6 bits 32 to
/// (32 << 2) | (32 >> 4) = 130, 5 bits 1 to (1 << 3) | 0 = 8, and all ones
/// to 255.
#[test]
fn decode_endpoints_expand_by_bit_replication() {
    // 16 << 11 | 32 << 5 | 1 = 0x8401.
    let level = one_level(&Tex::dxt1(4, 4, colour(0x8401, 0x0000, [0x04, 0, 0, 0])));
    let a = [132, 130, 8, 255];
    let black = [0, 0, 0, 255];
    let expected = flat(&[
        a, black, a, a, //
        a, a, a, a, //
        a, a, a, a, //
        a, a, a, a,
    ]);
    assert_eq!(level.rgba, expected);
    let white = one_level(&Tex::dxt1(4, 4, colour(0xFFFF, 0x0000, [0; 4])));
    assert_eq!(white.rgba, flat(&[[255, 255, 255, 255]; 16]));
}

/// FMT-065: with the endpoint 8 and 0, the interpolants are 16 / 3 = 5.33,
/// rounded to 5, and 8 / 3 = 2.67, rounded to 3 (truncation gives 2).
#[test]
fn decode_interpolated_colours_round_to_nearest() {
    let level = one_level(&Tex::dxt1(4, 4, colour(0x0800, 0x0000, [0xE4, 0, 0, 0])));
    let e0 = [8, 0, 0, 255];
    let expected = flat(&[
        e0,
        [0, 0, 0, 255],
        [5, 0, 0, 255],
        [3, 0, 0, 255], //
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
    ]);
    assert_eq!(level.rgba, expected);
}

/// FMT-065: the midpoint of 8 and 33 is 41 / 2 = 20.5, which rounds up to 21.
#[test]
fn decode_midpoint_tie_rounds_up() {
    // 4 << 11 = 0x2000 expands to (4 << 3) | (4 >> 2) = 33.
    let level = one_level(&Tex::dxt1(4, 4, colour(0x0800, 0x2000, [0xE4, 0, 0, 0])));
    let e0 = [8, 0, 0, 255];
    let expected = flat(&[
        e0,
        [33, 0, 0, 255],
        [21, 0, 0, 255],
        CLEAR, //
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
        e0,
    ]);
    assert_eq!(level.rgba, expected);
}

/// FMT-067: DXT3 alpha is the nibble times 17; texel 0 is the low nibble of
/// the first byte.
#[test]
fn decode_dxt3_alpha_is_a_nibble_times_17() {
    let alpha = [0x10, 0xF2, 0x88, 0x88, 0x88, 0x88, 0x88, 0x88];
    let level = one_level(&Tex::dxt3(
        4,
        4,
        dxt3_block(alpha, &colour(0xF800, 0x001F, [0; 4])),
    ));
    let px = |a| [255, 0, 0, a];
    let expected = flat(&[
        px(0),
        px(17),
        px(34),
        px(255),
        px(136),
        px(136),
        px(136),
        px(136),
        px(136),
        px(136),
        px(136),
        px(136),
        px(136),
        px(136),
        px(136),
        px(136),
    ]);
    assert_eq!(level.rgba, expected);
}

/// The four colours of a block whose first endpoint is blue and second red:
/// code 2 is (0 * 2 + 255 + 1) / 3 = 85 red and (2 * 255 + 1) / 3 = 170
/// blue, code 3 the reverse.
fn reversed_order_expected() -> Vec<u8> {
    let (e0, e1) = ([0, 0, 255, 255], [255, 0, 0, 255]);
    let (c2, c3) = ([85, 0, 170, 255], [170, 0, 85, 255]);
    flat(&[
        e0, e1, c2, c3, //
        c3, c2, e1, e0, //
        e0, e0, e0, e0, //
        c3, c3, c3, c3,
    ])
}

/// FMT-067: a DXT3 colour block with `color0 <= color1` still has four
/// colours.
#[test]
fn decode_dxt3_colour_block_ignores_endpoint_order() {
    let block = dxt3_block([0xFF; 8], &colour(0x001F, 0xF800, GRID));
    assert_eq!(
        one_level(&Tex::dxt3(4, 4, block)).rgba,
        reversed_order_expected()
    );
}

/// FMT-068: a DXT5 colour block with `color0 <= color1` still has four
/// colours.
#[test]
fn decode_dxt5_colour_block_ignores_endpoint_order() {
    let block = dxt5_block(255, 0, [0; 6], &colour(0x001F, 0xF800, GRID));
    assert_eq!(
        one_level(&Tex::dxt5(4, 4, block)).rgba,
        reversed_order_expected()
    );
}

/// Three-bit codes 0 to 7 twice over, the 48 bits of FMT-061: the first
/// 24 bits are 0xFAC688 (bytes 88 C6 FA), and so are the second.
const CODES_0_TO_7_TWICE: [u8; 6] = [0x88, 0xC6, 0xFA, 0x88, 0xC6, 0xFA];

fn dxt5_alpha_rows(a0: u8, a1: u8, rows: [[u8; 4]; 4]) {
    let block = dxt5_block(a0, a1, CODES_0_TO_7_TWICE, &colour(0xF800, 0x001F, [0; 4]));
    let level = one_level(&Tex::dxt5(4, 4, block));
    let expected: Vec<u8> = rows
        .iter()
        .flatten()
        .flat_map(|&a| [255, 0, 0, a])
        .collect();
    assert_eq!(level.rgba, expected, "alpha endpoints {a0} and {a1}");
}

/// FMT-068: 10 > 0, so codes 2 to 7 are (60, 50, 40, 30, 20, 10 plus 3) / 7
/// = 9, 7, 6, 4, 3, 1 (truncation gives 8, 7, 5, 4, 2, 1).
#[test]
fn decode_dxt5_six_interpolated_alphas_when_first_exceeds_second() {
    dxt5_alpha_rows(
        10,
        0,
        [[10, 0, 9, 7], [6, 4, 3, 1], [10, 0, 9, 7], [6, 4, 3, 1]],
    );
}

/// FMT-068: 0 <= 7, so codes 2 to 5 are (7, 14, 21, 28 plus 2) / 5 = 1, 3,
/// 4, 6 (truncation gives 1, 2, 4, 5), code 6 is 0 and code 7 is 255.
#[test]
fn decode_dxt5_four_interpolated_alphas_with_zero_and_opaque() {
    dxt5_alpha_rows(
        0,
        7,
        [[0, 7, 1, 3], [4, 6, 0, 255], [0, 7, 1, 3], [4, 6, 0, 255]],
    );
}

/// FMT-068: equal alpha bytes are not `alpha0 > alpha1`: the four-alpha
/// form, whose interpolants are all 5, with code 6 giving 0 and 7 giving 255.
#[test]
fn decode_dxt5_equal_alpha_endpoints_use_the_four_alpha_form() {
    dxt5_alpha_rows(
        5,
        5,
        [[5, 5, 5, 5], [5, 5, 0, 255], [5, 5, 5, 5], [5, 5, 0, 255]],
    );
}

/// The four pixels of FMT-069's worked texture, as bytes of a little-endian
/// word each.
const RAW_PIXELS: [u8; 16] = [
    10, 20, 30, 40, 11, 21, 31, 41, 1, 2, 3, 4, 255, 254, 253, 252,
];
/// Red in the top byte, green in the bottom, blue in the third, alpha in the
/// second.
const SHUFFLED: [u32; 4] = [0xFF00_0000, 0x0000_00FF, 0x00FF_0000, 0x0000_FF00];

/// FMT-069: red is byte 3 of the word, green byte 0, blue byte 2, alpha
/// byte 1.
#[test]
fn decode_uncompressed_reads_channels_through_the_masks() {
    let level = one_level(&Tex::raw32(2, 2, SHUFFLED, RAW_PIXELS.to_vec()));
    assert_eq!((level.width, level.height), (2, 2));
    let expected = flat(&[
        [40, 10, 30, 20],
        [41, 11, 31, 21],
        [4, 1, 3, 2],
        [252, 255, 253, 254],
    ]);
    assert_eq!(level.rgba, expected);
}

/// FMT-069: a zero alpha mask reads alpha as 255.
#[test]
fn decode_uncompressed_zero_alpha_mask_reads_opaque() {
    let masks = [SHUFFLED[0], SHUFFLED[1], SHUFFLED[2], 0];
    let level = one_level(&Tex::raw32(2, 2, masks, RAW_PIXELS.to_vec()));
    let expected = flat(&[
        [40, 10, 30, 255],
        [41, 11, 31, 255],
        [4, 1, 3, 255],
        [252, 255, 253, 255],
    ]);
    assert_eq!(level.rgba, expected);
}

fn raw_with_masks(masks: [u32; 4]) -> Tex {
    Tex::raw32(2, 2, masks, vec![0; 16])
}

/// FMT-069: a mask that is two bytes, half a byte off the boundary, empty
/// (for red, green, or blue), or two separate bytes is refused with the
/// channel and the mask.
#[test]
fn decode_uncompressed_refuses_masks_that_are_not_one_aligned_byte() {
    let [r, g, b, a] = USUAL_MASKS;
    for (masks, channel, mask) in [
        ([0x0000_FFFF, g, b, a], 'R', 0x0000_FFFF),
        ([r, 0, b, a], 'G', 0),
        ([r, g, 0x0000_0FF0, a], 'B', 0x0000_0FF0),
        ([r, g, b, 0x00FF_00FF], 'A', 0x00FF_00FF),
    ] {
        assert_eq!(
            refused(&raw_with_masks(masks)),
            Error::BadChannelMask { channel, mask }
        );
    }
}

/// FMT-069: two channels on one byte are refused, alpha included.
#[test]
fn decode_uncompressed_refuses_overlapping_masks() {
    let [r, g, b, _] = USUAL_MASKS;
    assert_eq!(
        refused(&raw_with_masks([r, r, b, 0])),
        Error::OverlappingChannelMasks { mask: r }
    );
    assert_eq!(
        refused(&raw_with_masks([r, g, b, b])),
        Error::OverlappingChannelMasks { mask: b }
    );
}

/// A format's name, its 2 by 2, 1 by 1, and 4 by 2 textures, and what each
/// decodes to.
type SmallCase = (&'static str, Tex, Tex, Tex, [Vec<u8>; 3]);

/// FMT-070: a level under four texels a side reads the texel indices 4 * y +
/// x of its one block.
#[test]
fn decode_small_levels_crop_one_block() {
    let dxt1 = colour(0xF800, 0x001F, GRID);
    let dxt3 = dxt3_block(
        [0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE],
        &colour(0xF800, 0x001F, [0; 4]),
    );
    let dxt5 = dxt5_block(14, 0, CODES_0_TO_7_TWICE, &colour(0xF800, 0x001F, [0; 4]));
    let red = |a| [255, 0, 0, a];

    let cases: [SmallCase; 3] = [
        (
            "dxt1",
            Tex::dxt1(2, 2, dxt1.clone()),
            Tex::dxt1(1, 1, dxt1.clone()),
            Tex::dxt1(4, 2, dxt1),
            [
                flat(&[RED, BLUE, ONE_THIRD, TWO_THIRDS]),
                flat(&[RED]),
                flat(&[
                    RED, BLUE, TWO_THIRDS, ONE_THIRD, //
                    ONE_THIRD, TWO_THIRDS, BLUE, RED,
                ]),
            ],
        ),
        (
            "dxt3",
            Tex::dxt3(2, 2, dxt3.clone()),
            Tex::dxt3(1, 1, dxt3.clone()),
            Tex::dxt3(4, 2, dxt3),
            [
                flat(&[red(0), red(17), red(68), red(85)]),
                flat(&[red(0)]),
                flat(&[
                    red(0),
                    red(17),
                    red(34),
                    red(51),
                    red(68),
                    red(85),
                    red(102),
                    red(119),
                ]),
            ],
        ),
        (
            "dxt5",
            Tex::dxt5(2, 2, dxt5.clone()),
            Tex::dxt5(1, 1, dxt5.clone()),
            Tex::dxt5(4, 2, dxt5),
            [
                flat(&[red(14), red(0), red(8), red(6)]),
                flat(&[red(14)]),
                flat(&[
                    red(14),
                    red(0),
                    red(12),
                    red(10),
                    red(8),
                    red(6),
                    red(4),
                    red(2),
                ]),
            ],
        ),
    ];
    for (name, two_by_two, one_by_one, four_by_two, [a, b, c]) in cases {
        let (la, lb, lc) = (
            one_level(&two_by_two),
            one_level(&one_by_one),
            one_level(&four_by_two),
        );
        assert_eq!(((la.width, la.height), la.rgba), ((2, 2), a), "{name} 2x2");
        assert_eq!(((lb.width, lb.height), lb.rgba), ((1, 1), b), "{name} 1x1");
        assert_eq!(((lc.width, lc.height), lc.rgba), ((4, 2), c), "{name} 4x2");
    }
}

fn solid(c0: u16) -> Vec<u8> {
    colour(c0, 0x0000, [0; 4])
}

/// FMT-064: the chain is the sum over levels of blocks of 8 or 16 bytes, and
/// one byte fewer is truncated, reporting the whole file it needed.
#[test]
fn decode_chain_length_is_the_sum_over_levels() {
    // 8x8 DXT5: 4 + 1 + 1 + 1 blocks of 16 bytes = 112.
    let full = Tex::dxt5(8, 8, vec![0; 112]).with_mips(4);
    assert_eq!(decoded(&full).len(), 4);
    let short = Tex::dxt5(8, 8, vec![0; 111]).with_mips(4);
    assert_eq!(
        refused(&short),
        Error::Truncated {
            needed: HEADER_LEN + 112,
            len: HEADER_LEN + 111
        }
    );
    // 5x5 DXT1: 4 + 1 + 1 blocks of 8 bytes = 48.
    assert_eq!(decoded(&Tex::dxt1(5, 5, vec![0; 48]).with_mips(3)).len(), 3);
    assert_eq!(
        refused(&Tex::dxt1(5, 5, vec![0; 47]).with_mips(3)),
        Error::Truncated {
            needed: HEADER_LEN + 48,
            len: HEADER_LEN + 47
        }
    );
    // An uncompressed chain is the sum of w * h * 4: 16 + 4 = 20.
    let raw = Tex::raw32(2, 2, USUAL_MASKS, vec![0; 20]).with_mips(2);
    assert_eq!(decoded(&raw).len(), 2);
    assert_eq!(
        refused(&Tex::raw32(2, 2, USUAL_MASKS, vec![0; 19]).with_mips(2)),
        Error::Truncated {
            needed: HEADER_LEN + 20,
            len: HEADER_LEN + 19
        }
    );
}

/// FMT-071: the pure size function gives the decoded bytes of every level:
/// 8x8 (256) + 4x4 (64) + 2x2 (16) + 1x1 (4) = 340; 5x5 (100) + 2x2 (16) +
/// 1x1 (4) = 120; 8x2 (64) + 4x1 (16) + 2x1 (8) + 1x1 (4) = 92.
#[test]
fn decoded_len_is_the_sum_of_the_level_sizes() {
    let sizes = [
        (Tex::dxt5(8, 8, vec![0; 112]).with_mips(4), 340),
        (Tex::dxt1(5, 5, vec![0; 48]).with_mips(3), 120),
        (Tex::dxt1(8, 2, vec![0; 40]).with_mips(4), 92),
        (Tex::raw32(2, 2, USUAL_MASKS, vec![0; 20]).with_mips(2), 20),
    ];
    for (tex, size) in sizes {
        assert_eq!(decoded_len(&tex.bytes()), Ok(size));
        let total: usize = decoded(&tex).iter().map(|l| l.rgba.len()).sum();
        assert_eq!(total, size);
    }
    let dims: Vec<(u32, u32)> = decoded(&Tex::dxt1(8, 2, vec![0; 40]).with_mips(4))
        .iter()
        .map(|l| (l.width, l.height))
        .collect();
    assert_eq!(dims, [(8, 2), (4, 1), (2, 1), (1, 1)]);
}

/// FMT-064: levels come top first, each from its own blocks in order.
#[test]
fn decode_levels_halve_down_to_one_pixel_in_file_order() {
    let data = [solid(0xF800), solid(0x07E0), solid(0x001F)].concat();
    let levels = decoded(&Tex::dxt1(4, 4, data).with_mips(3));
    let shapes: Vec<(u32, u32, Vec<u8>)> = levels
        .into_iter()
        .map(|l| (l.width, l.height, l.rgba))
        .collect();
    assert_eq!(
        shapes,
        [
            (4, 4, flat(&[RED; 16])),
            (2, 2, flat(&[[0, 255, 0, 255]; 4])),
            (1, 1, flat(&[BLUE])),
        ]
    );
}

/// FMT-064: the first stored row is the first row of the level; nothing is
/// flipped. Two blocks, red above blue, give four red rows then four blue.
#[test]
fn decode_rows_are_in_stored_order_without_a_flip() {
    let data = [solid(0xF800), solid(0x001F)].concat();
    let level = one_level(&Tex::dxt1(4, 8, data));
    let expected = flat(&[[RED; 16].as_slice(), [BLUE; 16].as_slice()].concat());
    assert_eq!(level.rgba, expected);
}

/// FMT-064: bytes after the chain change nothing.
#[test]
fn decode_ignores_bytes_after_the_chain() {
    let exact = Tex::dxt1(4, 4, colour(0xF800, 0x001F, GRID));
    let mut padded = exact.bytes();
    padded.extend_from_slice(&[9, 9, 9, 9, 9]);
    assert_eq!(decode(&padded), decode(&exact.bytes()));
    assert_eq!(decoded_len(&padded), Ok(64));
}

/// FMT-071: a zero side, a side over 8,192, and a word of all ones are
/// refused with both sides.
#[test]
fn decode_refuses_zero_and_oversized_dimensions() {
    for (width, height) in [(0, 4), (4, 0), (8193, 4), (4, 8193), (u32::MAX, 4)] {
        assert_eq!(
            refused(&Tex::dxt1(width, height, vec![0; 64])),
            Error::BadDimensions { width, height }
        );
    }
}

/// FMT-071: no mip count of zero, and none beyond one more than the base-2
/// logarithm of the longer side (4 -> 3, 2048 -> 12, 1 -> 1).
#[test]
fn decode_refuses_bad_mip_counts() {
    for (width, height, mips, most) in [
        (4, 4, 0, 3),
        (4, 4, 4, 3),
        (2048, 1024, 13, 12),
        (1, 1, 2, 1),
        (4, 4, u32::MAX, 3),
    ] {
        assert_eq!(
            refused(&Tex::dxt1(width, height, vec![0; 64]).with_mips(mips)),
            Error::BadMipCount {
                mip_count: mips,
                most
            }
        );
    }
    // The limits themselves are accepted: 3 levels of a 4x4 texture.
    assert_eq!(decoded(&Tex::dxt1(4, 4, vec![0; 24]).with_mips(3)).len(), 3);
}

/// FMT-071: cube maps and volume textures are refused by name.
#[test]
fn decode_refuses_cube_and_volume_textures() {
    for (caps2, kind) in [(0x200, "cube map"), (0x0020_0000, "volume texture")] {
        let tex = Tex {
            caps2,
            ..Tex::dxt1(4, 4, vec![0; 8])
        };
        assert_eq!(refused(&tex), Error::UnsupportedKind { kind });
    }
}

/// FMT-071: any other FOURCC and any other bit count is refused with what
/// was found.
#[test]
fn decode_refuses_other_formats() {
    for fourcc in [*b"ATI2", *b"DX10"] {
        assert_eq!(
            refused(&Tex::block(fourcc, 4, 4, vec![0; 16])),
            Error::UnsupportedFormat {
                found: Format::Other { fourcc }
            }
        );
    }
    for bits in [8, 16, 24] {
        let tex = Tex {
            bits,
            ..Tex::raw32(2, 2, USUAL_MASKS, vec![0; 16])
        };
        assert_eq!(
            refused(&tex),
            Error::UnsupportedFormat {
                found: Format::Uncompressed {
                    bits_per_pixel: bits
                }
            }
        );
    }
}

/// FMT-071: the largest allowed texture with no data is truncated, naming
/// the file it would need; nothing of that size is allocated first.
#[test]
fn decode_refuses_data_shorter_than_the_chain() {
    // 2048 * 2048 blocks of 8 bytes = 33,554,432 bytes.
    assert_eq!(
        refused(&Tex::dxt1(8192, 8192, Vec::new())),
        Error::Truncated {
            needed: HEADER_LEN + 33_554_432,
            len: HEADER_LEN
        }
    );
}

/// FMT-071: every proper prefix of a valid file is an error, a short header
/// reporting the 128 bytes it needs and a short chain the whole file.
#[test]
fn decode_every_prefix_of_a_valid_file_is_an_error() {
    let data: Vec<u8> = (0..112u8).map(|i| i.wrapping_mul(37)).collect();
    let file = Tex::dxt5(8, 8, data).with_mips(4).bytes();
    assert_eq!(file.len(), HEADER_LEN + 112);
    assert!(decode(&file).is_ok(), "the whole file decodes");
    for len in 0..file.len() {
        let needed = if len < HEADER_LEN {
            HEADER_LEN
        } else {
            file.len()
        };
        let expected = Error::Truncated { needed, len };
        assert_eq!(decode(&file[..len]), Err(expected.clone()), "prefix {len}");
        assert_eq!(decoded_len(&file[..len]), Err(expected), "prefix {len}");
    }
}

/// A fixed-seed xorshift generator, so a failing header can be rebuilt.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// 0, 1, 2, 2^31, the largest word, or a word of the generator's own.
    fn word(&mut self) -> u32 {
        match self.below(6) {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 1 << 31,
            4 => u32::MAX,
            _ => u32::try_from(self.next() >> 32).expect("the top half is a word"),
        }
    }
}

/// FMT-072: 4,000 headers of arbitrary words reach the size function; it
/// answers an error or at most 8 bytes for each byte of data, and the decoder
/// agrees with it exactly. At least 100 reach a size.
#[test]
fn decode_arbitrary_headers_reach_the_size_function_and_agree() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut reached = 0;
    for _ in 0..4000 {
        let (width, height) = (rng.word(), rng.word());
        let mips = rng.word();
        let len = usize::try_from(rng.below(4097)).expect("a small length");
        let data: Vec<u8> = (0..len)
            .map(|_| u8::try_from(rng.below(256)).expect("a byte"))
            .collect();
        let mut tex = match rng.below(5) {
            0 => Tex::dxt1(width, height, data),
            1 => Tex::dxt3(width, height, data),
            2 => Tex::dxt5(width, height, data),
            3 => Tex::raw32(width, height, USUAL_MASKS, data),
            _ => Tex::block(*b"ATI2", width, height, data),
        };
        if rng.below(2) == 0 {
            tex.mips = Some(mips);
        }
        let bytes = tex.bytes();
        let sized = decoded_len(&bytes);
        let result = decode(&bytes);
        assert_eq!(
            sized.is_ok(),
            result.is_ok(),
            "size function and decoder disagree for {width}x{height}: {sized:?}"
        );
        match (sized, result) {
            (Ok(size), Ok(levels)) => {
                reached += 1;
                assert!(size <= 8 * len, "{size} bytes from {len} bytes of data");
                let total: usize = levels.iter().map(|l| l.rgba.len()).sum();
                assert_eq!(total, size, "{width}x{height} with {mips} mips");
            }
            (Err(a), Err(b)) => assert_eq!(a, b),
            _ => {}
        }
    }
    assert!(reached >= 100, "only {reached} headers reached a size");
}

/// FMT-082: a block that overhangs the right or the bottom edge of a level
/// of any size is cropped to the level's width and height. A 6 by 5 level of
/// 2 by 2 blocks: block 0 red, block 1 blue and black on its first two
/// columns, block 2 green, block 3 white and black. A reader that wrote the
/// overhang would put block 1's third and fourth columns over the first two
/// pixels of the next row. The path is shared by the three block formats.
#[test]
fn decode_blocks_overhanging_the_level_are_cropped() {
    let data = [
        solid(0xF800),
        colour(0x001F, 0x0000, [0xE4; 4]),
        solid(0x07E0),
        colour(0xFFFF, 0x0000, [0xE4; 4]),
    ]
    .concat();
    let level = one_level(&Tex::dxt1(6, 5, data));
    assert_eq!((level.width, level.height), (6, 5));
    let (black, green, white) = ([0, 0, 0, 255], [0, 255, 0, 255], [255, 255, 255, 255]);
    let mut pixels = Vec::new();
    for _ in 0..4 {
        pixels.extend_from_slice(&[RED, RED, RED, RED, BLUE, black]);
    }
    pixels.extend_from_slice(&[green, green, green, green, white, black]);
    assert_eq!(level.rgba, flat(&pixels));
}

/// FMT-071: the checks run in the stated order, so a file with two faults
/// is refused for the one checked first.
#[test]
fn decode_refusals_follow_the_order_of_the_checks() {
    assert_eq!(
        refused(&Tex::dxt1(0, 4, vec![0; 8]).with_mips(0)),
        Error::BadDimensions {
            width: 0,
            height: 4
        }
    );
    assert_eq!(
        refused(&Tex {
            caps2: 0x200,
            ..Tex::dxt1(4, 4, vec![0; 8]).with_mips(4)
        }),
        Error::BadMipCount {
            mip_count: 4,
            most: 3
        }
    );
    for (caps2, kind) in [(0x200, "cube map"), (0x0020_0000, "volume texture")] {
        assert_eq!(
            refused(&Tex {
                caps2,
                ..Tex::block(*b"ATI2", 4, 4, vec![0; 16])
            }),
            Error::UnsupportedKind { kind }
        );
    }
    assert_eq!(
        refused(&Tex::block(*b"ATI2", 4, 4, Vec::new())),
        Error::UnsupportedFormat {
            found: Format::Other { fourcc: *b"ATI2" }
        }
    );
    assert_eq!(
        refused(&Tex::raw32(
            2,
            2,
            [0x0000_0F0F, 0x0000_FF00, 0x00FF_0000, 0],
            Vec::new()
        )),
        Error::BadChannelMask {
            channel: 'R',
            mask: 0x0000_0F0F
        }
    );
}
