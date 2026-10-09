//! Hermetic behaviour of `supercom-dds` over headers built in code. No game
//! is needed and no fixture file is committed: every texture here is a
//! 128-byte header assembled from literals, followed by zero or more opaque
//! data bytes.
//!
//! These pin what the installed game cannot: the exact error for each way a
//! header can be wrong, the classification of every pixel format, and the
//! meaning of the mip-count flag. The crate is a skeleton, so each test
//! below fails today on `Error::Unimplemented` and passes only when the
//! parser exists.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_dds::{Error, Format, Header};

const HEADER_LEN: usize = 128;

/// `DDSD_CAPS | DDSD_HEIGHT | DDSD_WIDTH | DDSD_PIXELFORMAT`, the flags every
/// texture carries.
const BASE_FLAGS: u32 = 0x1 | 0x2 | 0x4 | 0x1000;
/// `DDSD_MIPMAPCOUNT`: the mip-count field is meaningful.
const MIPMAPCOUNT: u32 = 0x2_0000;
/// `DDPF_FOURCC`: the pixel format carries a FOURCC.
const FOURCC: u32 = 0x4;
/// `DDPF_RGB`: the pixel format carries an RGB bit count.
const RGB: u32 = 0x40;
/// `DDPF_ALPHAPIXELS`.
const ALPHA: u32 = 0x1;

/// The fields of a synthetic header that the parser reads.
struct Spec {
    flags: u32,
    height: u32,
    width: u32,
    mips: u32,
    pf_flags: u32,
    fourcc: [u8; 4],
    bits: u32,
    data: usize,
}

impl Spec {
    /// A 4x4 DXT1 texture of one level with its 8 bytes of data.
    fn dxt1() -> Self {
        Self {
            flags: BASE_FLAGS,
            height: 4,
            width: 4,
            mips: 0,
            pf_flags: FOURCC,
            fourcc: *b"DXT1",
            bits: 0,
            data: 8,
        }
    }

    /// The file: the 128-byte header, then `data` zero bytes.
    fn bytes(&self) -> Vec<u8> {
        let mut out = b"DDS ".to_vec();
        for word in [
            124, // dwSize
            self.flags,
            self.height,
            self.width,
            0, // pitch or linear size
            0, // depth
            self.mips,
        ] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.resize(76, 0); // dwReserved1[11]
        out.extend_from_slice(&32u32.to_le_bytes()); // pixel format dwSize
        out.extend_from_slice(&self.pf_flags.to_le_bytes());
        out.extend_from_slice(&self.fourcc);
        out.extend_from_slice(&self.bits.to_le_bytes());
        out.resize(HEADER_LEN, 0); // masks, caps, reserved
        out.resize(HEADER_LEN + self.data, 0);
        out
    }
}

fn parsed(bytes: &[u8]) -> Header {
    let header = Header::parse(bytes);
    assert!(
        header.is_ok(),
        "Header::parse must succeed, got {:?}",
        header.as_ref().err()
    );
    header.expect("parsed: asserted above")
}

/// Pins `Error::BadMagic`: a full-length header that does not start with
/// `DDS ` is rejected with the four bytes it does start with.
#[test]
fn dds_wrong_magic_reports_the_found_bytes() {
    for magic in [*b"XXXX", *b"RIFF"] {
        let mut buf = Spec::dxt1().bytes();
        buf[..4].copy_from_slice(&magic);
        let result = Header::parse(&buf);
        assert!(
            matches!(result, Err(Error::BadMagic { found }) if found == magic),
            "{magic:?}: got {:?}",
            result.err()
        );
    }
}

/// Pins `Error::Truncated`: a buffer shorter than the 128-byte header
/// reports needing exactly 128 bytes and the length it has, whether it is
/// empty, carries only the magic, or lacks one byte.
#[test]
fn dds_header_shorter_than_128_bytes_is_truncated() {
    let full = Spec::dxt1().bytes();
    for len in [0, 4, 127] {
        let result = Header::parse(&full[..len]);
        assert!(
            matches!(result, Err(Error::Truncated { needed: 128, len: l }) if l == len),
            "{len} bytes: got {:?}",
            result.err()
        );
    }
}

/// Pins `Error::Malformed`: `dwSize` is 124 in every installed texture, and
/// a header that says otherwise is rejected.
#[test]
fn dds_dwsize_other_than_124_is_malformed() {
    let mut buf = Spec::dxt1().bytes();
    buf[4..8].copy_from_slice(&100u32.to_le_bytes());
    let result = Header::parse(&buf);
    assert!(
        matches!(result, Err(Error::Malformed { .. })),
        "got {:?}",
        result.err()
    );
}

/// Pins the FOURCC classification: `DXT1`, `DXT3`, and `DXT5` map to their
/// formats.
#[test]
fn dds_dxt_fourcc_classifies_as_the_compressed_format() {
    for (fourcc, expected) in [
        (*b"DXT1", Format::Dxt1),
        (*b"DXT3", Format::Dxt3),
        (*b"DXT5", Format::Dxt5),
    ] {
        let spec = Spec {
            fourcc,
            ..Spec::dxt1()
        };
        assert_eq!(parsed(&spec.bytes()).format, expected);
    }
}

/// Pins `Format::Other`: a FOURCC outside the three classified ones is
/// reported with its bytes, not coerced to a DXT format.
#[test]
fn dds_unclassified_fourcc_is_reported_with_its_bytes() {
    for fourcc in [*b"ATI2", *b"DX10"] {
        let spec = Spec {
            fourcc,
            ..Spec::dxt1()
        };
        assert_eq!(parsed(&spec.bytes()).format, Format::Other { fourcc });
    }
}

/// Pins the uncompressed classification: a pixel format without the FOURCC
/// flag is `Uncompressed` with the RGB bit count at offset 88 (32, 24, and 8
/// bits occur in the install), and FOURCC bytes left in the header are
/// ignored while the flag is clear.
#[test]
fn dds_without_the_fourcc_flag_is_uncompressed_with_its_bit_depth() {
    for (pf_flags, bits) in [(RGB | ALPHA, 32), (RGB, 24), (RGB, 8)] {
        let spec = Spec {
            pf_flags,
            bits,
            fourcc: *b"DXT5",
            ..Spec::dxt1()
        };
        assert_eq!(
            parsed(&spec.bytes()).format,
            Format::Uncompressed {
                bits_per_pixel: bits
            }
        );
    }
}

/// Pins the mip-count flag: with `DDSD_MIPMAPCOUNT` unset the header's mip
/// field is ignored and `mip_count` is 1, even when the field holds 7.
#[test]
fn dds_without_the_mip_count_flag_reports_one_mip() {
    for mips in [0, 1, 7] {
        let spec = Spec {
            flags: BASE_FLAGS,
            mips,
            ..Spec::dxt1()
        };
        assert_eq!(parsed(&spec.bytes()).mip_count, 1, "field held {mips}");
    }
}

/// Pins the mip-count flag: with `DDSD_MIPMAPCOUNT` set, `mip_count` is the
/// header's field.
#[test]
fn dds_with_the_mip_count_flag_reports_the_field() {
    for mips in [2, 7, 13] {
        let spec = Spec {
            flags: BASE_FLAGS | MIPMAPCOUNT,
            mips,
            ..Spec::dxt1()
        };
        assert_eq!(parsed(&spec.bytes()).mip_count, mips);
    }
}

/// Pins the dimension offsets: height is the word at offset 12 and width the
/// word at offset 16, so an 8-wide, 4-high texture reports 8 and 4, not the
/// reverse.
#[test]
fn dds_width_and_height_come_from_their_own_offsets() {
    let spec = Spec {
        width: 8,
        height: 4,
        data: 16,
        ..Spec::dxt1()
    };
    let header = parsed(&spec.bytes());
    assert_eq!((header.width, header.height), (8, 4));
}

/// Pins every field of the minimal texture, a 4x4 DXT1 of one level with its
/// 8 data bytes.
#[test]
fn dds_minimal_dxt1_texture_round_trips_its_fields() {
    let expected = Header {
        width: 4,
        height: 4,
        mip_count: 1,
        format: Format::Dxt1,
        data_len: 8,
    };
    assert_eq!(parsed(&Spec::dxt1().bytes()), expected);
}

/// Pins `data_len`: it is the number of bytes after the header, whatever the
/// texture's format claims: none, 100, and 112 (the four-level chain of an
/// 8x8 DXT5: 64 + 16 + 16 + 16 bytes).
#[test]
fn dds_data_len_counts_the_bytes_after_the_header() {
    for data in [0, 100, 112] {
        let spec = Spec {
            data,
            ..Spec::dxt1()
        };
        assert_eq!(parsed(&spec.bytes()).data_len, data);
    }
}

/// Pins that the pixel data is opaque: a header followed by 0xFF bytes parses
/// to the same `Header` as the same header followed by zeros.
#[test]
fn dds_header_does_not_depend_on_the_pixel_data() {
    let zeros = Spec::dxt1().bytes();
    let mut ones = zeros.clone();
    ones[HEADER_LEN..].fill(0xFF);
    assert_eq!(parsed(&ones), parsed(&zeros));
}
