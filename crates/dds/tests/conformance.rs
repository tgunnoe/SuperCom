//! Conformance of `supercom-dds` against the installed game.
//!
//! Every test here is game-dependent: it begins by asking `supercom_testkit`
//! for the install and returns (skips, loudly) when there is none. Run them
//! against the real game with `SUPERCOM_REQUIRE_GAME=1`.
//!
//! The expected numbers are measured on the installed Forged Alliance:
//! 14,275 `.dds` members, every one with a 128-byte header; 11,211 DXT5,
//! 1,250 DXT3, 1,242 DXT1, and 572 uncompressed. No game content is stored
//! here: counts and sizes only.
//!
//! Each test's first assertion is on the behaviour under test, never on the
//! environment, so a failure points at the capability and not at the install.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::BTreeMap;
use std::path::Path;

use supercom_dds::{Format, Header};
use supercom_scd::Vfs;

/// `.dds` members in the install.
const TEXTURES: usize = 14_275;
const HEADER_LEN: usize = 128;
/// `DDSD_MIPMAPCOUNT`.
const MIPMAPCOUNT: u32 = 0x2_0000;

/// Mounts the install, failing the calling test with the error if it cannot.
fn mounted(fa: &Path) -> Vfs {
    let vfs = Vfs::from_install(fa);
    assert!(
        vfs.is_ok(),
        "Vfs::from_install must mount the install, got {:?}",
        vfs.as_ref().err()
    );
    vfs.expect("mounted: asserted above")
}

/// The little-endian `u32` at `at`, read without the code under test.
fn le_u32(bytes: &[u8], at: usize) -> u32 {
    let word: [u8; 4] = bytes[at..at + 4].try_into().expect("four bytes");
    u32::from_le_bytes(word)
}

/// Parses every `.dds` member, failing the calling test at the first parse
/// error, and hands each (name, raw bytes, header) to `visit`. Returns how
/// many were visited. Only the header is kept: each file's bytes are dropped
/// before the next is read.
fn each_texture(vfs: &Vfs, mut visit: impl FnMut(&str, &[u8], &Header)) -> usize {
    let mut visited = 0;
    for entry in vfs.files() {
        if !entry.name.to_lowercase().ends_with(".dds") {
            continue;
        }
        let read = vfs.read(&entry.name);
        assert!(
            read.is_ok(),
            "{} unreadable: {:?}",
            entry.name,
            read.as_ref().err()
        );
        let bytes = read.expect("read: asserted above");
        let header = Header::parse(&bytes);
        assert!(
            header.is_ok(),
            "{} must parse, got {:?}",
            entry.name,
            header.as_ref().err()
        );
        visit(
            &entry.name,
            &bytes,
            &header.expect("parsed: asserted above"),
        );
        visited += 1;
    }
    visited
}

/// Bytes of one mip level of a block-compressed texture: 4x4 blocks, at
/// least one block each way.
fn level_len(width: u32, height: u32, block: u64) -> u64 {
    u64::from(width.div_ceil(4).max(1)) * u64::from(height.div_ceil(4).max(1)) * block
}

/// Bytes of all `mip_count` levels, each half the size of the last down to
/// 1x1; `None` for a format that is not block-compressed.
fn chain_len(header: &Header) -> Option<u64> {
    let block = match header.format {
        Format::Dxt1 => 8,
        Format::Dxt3 | Format::Dxt5 => 16,
        Format::Uncompressed { .. } | Format::Other { .. } => return None,
    };
    let (mut width, mut height) = (header.width, header.height);
    let mut total = 0;
    for _ in 0..header.mip_count {
        total += level_len(width, height, block);
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    Some(total)
}

/// Pins `Header::parse` over the install: all 14,275 `.dds` members parse,
/// `width` and `height` are the words at offsets 16 and 12, and `data_len` is
/// the file length minus the 128-byte header.
#[test]
fn conformance_dds_every_header_parses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let mut violations = Vec::new();
    let visited = each_texture(&vfs, |name, raw, header| {
        let ok = header.width == le_u32(raw, 16)
            && header.height == le_u32(raw, 12)
            && header.width > 0
            && header.height > 0
            && header.data_len == raw.len() - HEADER_LEN;
        if !ok {
            violations.push(name.to_owned());
        }
    });
    assert!(
        violations.is_empty(),
        "{} headers disagree with their bytes, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(visited, TEXTURES, "textures parsed");
}

/// Pins the format classification: exactly 11,211 `Dxt5`, 1,250 `Dxt3`,
/// 1,242 `Dxt1`, 572 `Uncompressed`, and 0 `Other`; the uncompressed ones are
/// 533 at 32 bits, 36 at 24 bits, and 3 at 8 bits.
#[test]
fn conformance_dds_format_census() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let mut census: BTreeMap<String, usize> = BTreeMap::new();
    each_texture(&vfs, |_, _, header| {
        let key = match header.format {
            Format::Dxt1 => "Dxt1".to_owned(),
            Format::Dxt3 => "Dxt3".to_owned(),
            Format::Dxt5 => "Dxt5".to_owned(),
            Format::Uncompressed { bits_per_pixel } => format!("Uncompressed {bits_per_pixel}"),
            Format::Other { .. } => "Other".to_owned(),
        };
        *census.entry(key).or_default() += 1;
    });
    let expected: BTreeMap<String, usize> = [
        ("Dxt1", 1242),
        ("Dxt3", 1250),
        ("Dxt5", 11_211),
        ("Uncompressed 32", 533),
        ("Uncompressed 24", 36),
        ("Uncompressed 8", 3),
    ]
    .into_iter()
    .map(|(key, count)| (key.to_owned(), count))
    .collect();
    assert_eq!(census, expected);
}

/// Pins the mip-count flag over the install: the 6,721 textures with
/// `DDSD_MIPMAPCOUNT` set report the header's field at offset 28 (always more
/// than 1), and the 7,554 without it report 1.
#[test]
fn conformance_dds_mip_count_follows_the_flag() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let (mut flagged, mut unflagged) = (0, 0);
    let mut violations = Vec::new();
    each_texture(&vfs, |name, raw, header| {
        let ok = if le_u32(raw, 8) & MIPMAPCOUNT == 0 {
            unflagged += 1;
            header.mip_count == 1
        } else {
            flagged += 1;
            header.mip_count == le_u32(raw, 28) && header.mip_count > 1
        };
        if !ok {
            violations.push(name.to_owned());
        }
    });
    assert!(
        violations.is_empty(),
        "{} textures report the wrong mip count, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(flagged, 6721, "textures with the mip-count flag");
    assert_eq!(unflagged, 7554, "textures without it");
}

/// Pins the data length against the mip chain: for each of the 13,703
/// block-compressed textures, the size of all `mip_count` levels (max(1,
/// ceil(w/4)) * max(1, ceil(h/4)) blocks of 8 bytes for DXT1 and 16 for
/// DXT3 and DXT5, levels halving to 1x1) is at most `data_len`, and equals it
/// in 13,620 of them.
#[test]
fn conformance_dds_data_fits() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let (mut compressed, mut exact) = (0, 0);
    let mut violations = Vec::new();
    each_texture(&vfs, |name, _, header| {
        let Some(chain) = chain_len(header) else {
            return;
        };
        compressed += 1;
        let data_len = u64::try_from(header.data_len).expect("a length fits in u64");
        if chain > data_len {
            violations.push(name.to_owned());
        } else if chain == data_len {
            exact += 1;
        }
    });
    assert!(
        violations.is_empty(),
        "{} textures are shorter than their mip chain, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(compressed, 13_703, "block-compressed textures");
    assert_eq!(exact, 13_620, "textures whose data is exactly the chain");
}
