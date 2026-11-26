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

use std::collections::HashMap;
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

/// What decoding one texture gives the digests of FMT-077: its CRC-32 over
/// every level's bytes, the levels and bytes it holds, and the DXT3 and DXT5
/// colour blocks that FMT-080 counts.
struct Decoded {
    crc: u32,
    levels: u64,
    bytes: u64,
    late_blocks: u64,
}

/// The table of the CRC-32 with the reflected polynomial 0xEDB88320.
fn crc_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    for (slot, index) in table.iter_mut().zip(0_u32..) {
        let mut crc = index;
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
        *slot = crc;
    }
    table
}

/// DXT3 and DXT5 colour blocks whose first endpoint is at or below the
/// second and which use a code of 2 or 3, counted over every level of the
/// chain, read from the raw bytes and not from the decoder.
fn late_colour_blocks(raw: &[u8], header: &Header) -> u64 {
    if !matches!(header.format, Format::Dxt3 | Format::Dxt5) {
        return 0;
    }
    let (mut width, mut height) = (header.width, header.height);
    let mut at = HEADER_LEN;
    let mut late = 0;
    for _ in 0..header.mip_count {
        let blocks = width.div_ceil(4).max(1) * height.div_ceil(4).max(1);
        for _ in 0..blocks {
            let block = &raw[at..at + 16];
            at += 16;
            let word = |from: usize| {
                u32::from_le_bytes([
                    block[from],
                    block[from + 1],
                    block[from + 2],
                    block[from + 3],
                ])
            };
            let (c0, c1) = (block[8..10].to_vec(), block[10..12].to_vec());
            let first = u16::from_le_bytes([c0[0], c0[1]]);
            let second = u16::from_le_bytes([c1[0], c1[1]]);
            let codes = word(12);
            let uses_late_code = (0..16).any(|texel| (codes >> (2 * texel)) & 3 >= 2);
            if first <= second && uses_late_code {
                late += 1;
            }
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    late
}

/// Decodes `raw` and digests its levels, top first, bytes in the order red,
/// green, blue, alpha.
fn decode_for_digest(raw: &[u8], table: &[u32; 256]) -> Decoded {
    let header = Header::parse(raw);
    assert!(
        header.is_ok(),
        "header must parse, got {:?}",
        header.as_ref().err()
    );
    let header = header.expect("parsed: asserted above");
    let levels = supercom_dds::decode(raw);
    assert!(
        levels.is_ok(),
        "texture must decode, got {:?}",
        levels.as_ref().err()
    );
    let levels = levels.expect("decoded: asserted above");
    let mut register = u32::MAX;
    let mut bytes = 0_u64;
    for level in &levels {
        for &byte in &level.rgba {
            register = table
                [usize::from(u8::try_from((register ^ u32::from(byte)) & 0xFF).expect("a byte"))]
                ^ (register >> 8);
        }
        bytes += u64::try_from(level.rgba.len()).expect("a length fits in u64");
    }
    Decoded {
        crc: !register,
        levels: u64::try_from(levels.len()).expect("a count fits in u64"),
        bytes,
        late_blocks: late_colour_blocks(raw, &header),
    }
}

/// The combined digest of FMT-077: FNV-1a over 64 bits of each file's CRC as
/// four little-endian bytes, in the order given.
fn combined_digest(files: &[Decoded]) -> u64 {
    let mut state: u64 = 0xcbf2_9ce4_8422_2325;
    for file in files {
        for byte in file.crc.to_le_bytes() {
            state = (state ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    state
}

/// Decodes the files `read` yields for each of `names`, several at a time,
/// holding at most one batch of raw files; the results are in `names`'
/// order.
fn decode_all(names: &[String], read: impl Fn(&str) -> Vec<u8> + Sync) -> Vec<Decoded> {
    let table = crc_table();
    let workers = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let mut out = Vec::with_capacity(names.len());
    for batch in names.chunks(workers * 4) {
        let size = batch.len().div_ceil(workers).max(1);
        let parts: Vec<Vec<Decoded>> = std::thread::scope(|scope| {
            let handles: Vec<_> = batch
                .chunks(size)
                .map(|part| {
                    let (read, table) = (&read, &table);
                    scope.spawn(move || {
                        part.iter()
                            .map(|name| decode_for_digest(&read(name), table))
                            .collect::<Vec<Decoded>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("a decoding thread must not panic"))
                .collect()
        });
        out.extend(parts.into_iter().flatten());
    }
    out
}

/// The totals a set's clause states: files, levels, bytes, late colour
/// blocks, and the combined digest.
fn totals(files: &[Decoded]) -> (usize, u64, u64, u64, u64) {
    (
        files.len(),
        files.iter().map(|f| f.levels).sum(),
        files.iter().map(|f| f.bytes).sum(),
        files.iter().map(|f| f.late_blocks).sum(),
        combined_digest(files),
    )
}

/// Pins the units' textures (FMT-079, FMT-080): for each of the 526 unit
/// blueprints that have both `units/<ID>/<ID>_albedo.dds` and
/// `units/<ID>/<ID>_specteam.dds`, 1,052 files in ascending order of path,
/// the decoded levels hold 10,473 levels and 3,079,054,692 bytes and have the
/// combined digest 9925447467578818197, and no DXT3 or DXT5 colour block has
/// its first endpoint at or below its second with a code of 2 or 3.
#[test]
fn conformance_dds_unit_textures_digest() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);
    let stored: HashMap<String, String> = vfs
        .files()
        .map(|entry| (entry.name.to_lowercase(), entry.name.clone()))
        .collect();
    let mut paths: Vec<String> = stored
        .keys()
        .filter_map(|name| {
            let id = name
                .strip_prefix("units/")?
                .strip_suffix("_unit.bp")?
                .rsplit('/')
                .next()?;
            Some(id.to_owned())
        })
        .flat_map(|id| {
            [
                format!("units/{id}/{id}_albedo.dds"),
                format!("units/{id}/{id}_specteam.dds"),
            ]
        })
        .collect();
    paths.sort();
    // A blueprint has the pair or contributes none of it.
    let held: std::collections::HashSet<String> = paths
        .iter()
        .filter(|p| stored.contains_key(*p))
        .cloned()
        .collect();
    paths.retain(|path| {
        let (albedo, specteam) = if path.ends_with("_albedo.dds") {
            (path.clone(), path.replace("_albedo.dds", "_specteam.dds"))
        } else {
            (path.replace("_specteam.dds", "_albedo.dds"), path.clone())
        };
        held.contains(&albedo) && held.contains(&specteam)
    });
    let files = decode_all(&paths, |path| {
        let name = &stored[path];
        let read = vfs.read(name);
        assert!(read.is_ok(), "{name} unreadable: {:?}", read.as_ref().err());
        read.expect("read: asserted above")
    });
    assert_eq!(
        totals(&files),
        (1052, 10_473, 3_079_054_692, 0, 9_925_447_467_578_818_197)
    );
}
