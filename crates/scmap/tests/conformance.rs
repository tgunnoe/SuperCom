//! Conformance of `supercom-scmap` against the installed game.
//!
//! Every test here is game-dependent: it begins by asking `supercom_testkit`
//! for the install and returns (skips, loudly) when there is none. Run them
//! against the real game with `SUPERCOM_REQUIRE_GAME=1`.
//!
//! The expected numbers are measured on the installed Forged Alliance: 60
//! `.scmap` files in 61 folders under `maps/`, all inner version 60, in six
//! size classes. Maps live outside the archives, at
//! `maps/<name>/<name>.scmap`; their strata textures live inside them. No
//! game content is stored here: counts and sizes only.
//!
//! Each test's first assertion is on the behaviour under test, never on the
//! environment, so a failure points at the capability and not at the install.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use supercom_dds::{Format, Header};
use supercom_scd::Vfs;
use supercom_scmap::Map;

/// `.scmap` files in the install.
const MAPS: usize = 60;
/// Bytes of a texture file's header.
const HEADER_LEN: usize = 128;

/// Every `.scmap` directly inside a folder of `<install>/maps`, in path
/// order, found without the code under test.
fn map_paths(fa: &Path) -> Vec<PathBuf> {
    let folders = fs::read_dir(fa.join("maps"));
    assert!(
        folders.is_ok(),
        "cannot list maps: {:?}",
        folders.as_ref().err()
    );
    let mut paths: Vec<PathBuf> = folders
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .flat_map(|dir| fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("scmap"))
        })
        .collect();
    paths.sort();
    paths
}

/// Parses every map, failing the calling test at the first parse error, and
/// hands each (path, map) to `visit`. Returns how many were visited.
fn each_map(fa: &Path, mut visit: impl FnMut(&Path, &Map)) -> usize {
    let mut visited = 0;
    for path in map_paths(fa) {
        let read = fs::read(&path);
        assert!(
            read.is_ok(),
            "{} unreadable: {:?}",
            path.display(),
            read.as_ref().err()
        );
        let map = Map::parse(&read.expect("read: asserted above"));
        assert!(
            map.is_ok(),
            "{} must parse, got {:?}",
            path.display(),
            map.as_ref().err()
        );
        visit(&path, &map.expect("parsed: asserted above"));
        visited += 1;
    }
    visited
}

fn as_usize(value: u32) -> usize {
    usize::try_from(value).expect("a u32 fits in usize")
}

/// A strata path as the Vfs names it: a leading `/` stripped, `\` mapped to
/// `/`, lowercased.
fn normalized(path: &str) -> String {
    path.strip_prefix('/')
        .unwrap_or(path)
        .replace('\\', "/")
        .to_lowercase()
}

/// Pins `Map::parse` over the install: all 60 maps parse, with inner version
/// 60, a world size in one of the six size classes (256, 512, 1024, 2048 by
/// 1024, 2048, and 4096 square), a heightmap grid equal to the world size,
/// and `(width + 1) * (height + 1)` samples.
#[test]
fn conformance_scmap_every_map_parses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };

    let classes: Vec<(u32, u32)> = [
        (256.0_f32, 256.0_f32),
        (512.0, 512.0),
        (1024.0, 1024.0),
        (2048.0, 1024.0),
        (2048.0, 2048.0),
        (4096.0, 4096.0),
    ]
    .iter()
    .map(|&(w, h)| (w.to_bits(), h.to_bits()))
    .collect();
    let mut violations = Vec::new();
    let visited = each_map(&fa, |path, map| {
        let hm = &map.heightmap;
        let ok = map.version == 60
            && classes.contains(&(map.width.to_bits(), map.height.to_bits()))
            && f64::from(map.width).to_bits() == f64::from(hm.width).to_bits()
            && f64::from(map.height).to_bits() == f64::from(hm.height).to_bits()
            && map.preview.len() > 128
            && hm.samples.len() == (as_usize(hm.width) + 1) * (as_usize(hm.height) + 1);
        if !ok {
            violations.push(path.display().to_string());
        }
    });
    assert!(
        violations.is_empty(),
        "{} maps are inconsistent, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(visited, MAPS, "maps parsed");
}

/// Pins the size classes: the 60 maps split exactly 10 at 256x256, 18 at
/// 512x512, 22 at 1024x1024, 1 at 2048x1024, 6 at 2048x2048, and 3 at
/// 4096x4096, by heightmap grid.
#[test]
fn conformance_scmap_size_class_census() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };

    let mut census: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    each_map(&fa, |_, map| {
        *census
            .entry((map.heightmap.width, map.heightmap.height))
            .or_default() += 1;
    });
    let expected: BTreeMap<(u32, u32), usize> = [
        ((256, 256), 10),
        ((512, 512), 18),
        ((1024, 1024), 22),
        ((2048, 1024), 1),
        ((2048, 2048), 6),
        ((4096, 4096), 3),
    ]
    .into_iter()
    .collect();
    assert_eq!(census, expected);
}

/// Pins the preview: in all 60 maps the embedded preview is a whole `.dds`
/// file, so `supercom_dds::Header::parse` accepts it, and its data is the
/// rest of the blob.
#[test]
fn conformance_scmap_preview_is_dds() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };

    let mut violations = Vec::new();
    let visited = each_map(&fa, |path, map| {
        let header = Header::parse(&map.preview);
        let ok = header
            .as_ref()
            .is_ok_and(|h| h.data_len == map.preview.len() - 128 && h.width > 0 && h.height > 0);
        if !ok {
            violations.push(format!("{}: {:?}", path.display(), header.err()));
        }
    });
    assert!(
        violations.is_empty(),
        "{} previews are not a whole DDS, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(visited, MAPS, "maps parsed");
}

/// Pins the strata: every map has ten stratum slots (600 in all), 170 of them
/// unused (empty path) and 430 named, and every named texture path, after
/// normalization (strip a leading `/`, map `\` to `/`, lowercase), is a member
/// of the install's virtual filesystem.
#[test]
fn conformance_scmap_strata_resolve() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = Vfs::from_install(&fa);
    assert!(
        vfs.is_ok(),
        "Vfs::from_install must mount the install, got {:?}",
        vfs.as_ref().err()
    );
    let vfs = vfs.expect("mounted: asserted above");
    let members: HashSet<String> = vfs.files().map(|e| e.name.to_lowercase()).collect();

    let (mut slots, mut empty, mut named) = (0, 0, 0);
    let mut unresolved = Vec::new();
    let mut wrong_count = Vec::new();
    each_map(&fa, |path, map| {
        if map.strata.len() != 10 {
            wrong_count.push(path.display().to_string());
        }
        for stratum in &map.strata {
            slots += 1;
            if stratum.texture.is_empty() {
                empty += 1;
                continue;
            }
            named += 1;
            if !members.contains(&normalized(&stratum.texture)) {
                unresolved.push(stratum.texture.clone());
            }
        }
    });
    assert!(
        wrong_count.is_empty(),
        "{} maps do not have ten strata, first: {:?}",
        wrong_count.len(),
        wrong_count.iter().take(3).collect::<Vec<_>>()
    );
    assert!(
        unresolved.is_empty(),
        "{} stratum textures are not in the Vfs, first: {:?}",
        unresolved.len(),
        unresolved.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(slots, 600, "stratum slots across 60 maps");
    assert_eq!(empty, 170, "unused slots");
    assert_eq!(named, 430, "named slots");
}

/// Pins the water census: 43 of the 60 maps carry water, with the surface
/// elevation from 0 to 30, the deep elevation at or below it and the abyss
/// at or below the deep; the other 17, named below by folder, carry none.
#[test]
fn conformance_scmap_water_census() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };

    let dry_folders = [
        "scmp_003", "scmp_004", "scmp_006", "scmp_007", "scmp_012", "scmp_013", "scmp_015",
        "scmp_017", "scmp_018", "scmp_022", "scmp_023", "scmp_026", "scmp_031", "scmp_036",
        "scmp_038", "x1mp_001", "x1mp_002",
    ];
    let mut wet = 0_usize;
    let mut dry = Vec::new();
    let mut violations = Vec::new();
    each_map(&fa, |path, map| {
        let folder = path
            .parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        match map.water {
            Some(water) => {
                wet += 1;
                let ok = (0.0..=30.0).contains(&water.elevation)
                    && water.deep <= water.elevation
                    && water.abyss <= water.deep;
                if !ok {
                    violations.push(format!("{folder}: {water:?}"));
                }
            }
            None => dry.push(folder),
        }
    });
    dry.sort();
    assert_eq!(wet, 43, "maps with water");
    assert_eq!(dry, dry_folders, "maps without water");
    assert!(
        violations.is_empty(),
        "{} water blocks out of range, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
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
    let mut state: u64 = 0xcbf2_9ce4_8422_5325;
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

/// Every map, in ascending order of path, with its parse.
fn all_maps(fa: &Path) -> Vec<(PathBuf, Map)> {
    let mut maps = Vec::new();
    each_map(fa, |path, map| maps.push((path.to_path_buf(), map.clone())));
    maps
}

/// Pins the ground textures (FMT-077, FMT-080): the 114 distinct files that
/// the 430 named strata slots name hold 1,020 levels and 160,772,940 bytes
/// decoded, the combined digest 5853751105962280521, and no late colour
/// block.
#[test]
fn conformance_scmap_ground_textures_digest() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = Vfs::from_install(&fa);
    assert!(vfs.is_ok(), "must mount, got {:?}", vfs.as_ref().err());
    let vfs = vfs.expect("mounted: asserted above");
    let names: BTreeSet<String> = all_maps(&fa)
        .iter()
        .flat_map(|(_, map)| map.strata.iter())
        .filter(|stratum| !stratum.texture.is_empty())
        .map(|stratum| normalized(&stratum.texture))
        .collect();
    let names: Vec<String> = names.into_iter().collect();
    let files = decode_all(&names, |name| {
        let read = vfs.read(name);
        assert!(read.is_ok(), "{name} unreadable: {:?}", read.as_ref().err());
        read.expect("read: asserted above")
    });
    assert_eq!(
        totals(&files),
        (114, 1020, 160_772_940, 0, 5_853_751_105_962_280_521)
    );
}

/// Pins the masks (FMT-078, FMT-080): the 120 masks, in ascending order of
/// map path, low before high, hold 120 levels and 212,074,496 bytes decoded,
/// the combined digest 9191786234201547060, and no late colour block.
#[test]
fn conformance_scmap_masks_digest() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let maps = all_maps(&fa);
    let blobs: Vec<&Vec<u8>> = maps
        .iter()
        .flat_map(|(_, map)| [&map.low_mask, &map.high_mask])
        .collect();
    let names: Vec<String> = (0..blobs.len()).map(|i| i.to_string()).collect();
    let files = decode_all(&names, |index| {
        let at: usize = index.parse().expect("an index");
        blobs[at].clone()
    });
    assert_eq!(
        totals(&files),
        (120, 120, 212_074_496, 0, 9_191_786_234_201_547_060)
    );
}

/// Pins the masks' census (FMT-081): each of the 120 masks is an
/// uncompressed 32-bit texture of one level whose width and height are half
/// the heightmap's cell counts, with no data after it; 40 maps have two
/// byte-equal masks, 41 name none of the strata slots 5 to 8, and the terrain
/// programs are `TTerrain` on 39, `TTerrainXP` on 20, and `TTerrainGlow` on 1.
#[test]
fn conformance_scmap_masks_census() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let maps = all_maps(&fa);
    let mut sizes: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    let mut violations = Vec::new();
    let (mut equal, mut none_of_five_to_eight) = (0, 0);
    let mut programs: BTreeMap<String, usize> = BTreeMap::new();
    for (path, map) in &maps {
        for mask in [&map.low_mask, &map.high_mask] {
            let header = Header::parse(mask);
            let expected = (map.heightmap.width / 2, map.heightmap.height / 2);
            let ok = header.as_ref().is_ok_and(|h| {
                h.format == Format::Uncompressed { bits_per_pixel: 32 }
                    && h.mip_count == 1
                    && (h.width, h.height) == expected
                    && as_usize(h.width) * as_usize(h.height) * 4 == h.data_len
            });
            if !ok {
                violations.push(format!("{}: {:?}", path.display(), header.err()));
            }
            *sizes.entry(expected).or_default() += 1;
        }
        equal += usize::from(map.low_mask == map.high_mask);
        none_of_five_to_eight +=
            usize::from(map.strata[5..=8].iter().all(|s| s.texture.is_empty()));
        *programs.entry(map.terrain_program.clone()).or_default() += 1;
    }
    assert!(
        violations.is_empty(),
        "{} masks differ, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(maps.len(), MAPS, "maps parsed");
    let expected_sizes: BTreeMap<(u32, u32), usize> = [
        ((128, 128), 20),
        ((256, 256), 36),
        ((512, 512), 44),
        ((1024, 512), 2),
        ((1024, 1024), 12),
        ((2048, 2048), 6),
    ]
    .into_iter()
    .collect();
    assert_eq!(sizes, expected_sizes);
    assert_eq!(equal, 40, "maps with byte-equal masks");
    assert_eq!(
        none_of_five_to_eight, 41,
        "maps naming none of slots 5 to 8"
    );
    let expected_programs: BTreeMap<String, usize> =
        [("TTerrain", 39), ("TTerrainGlow", 1), ("TTerrainXP", 20)]
            .into_iter()
            .map(|(name, count)| (name.to_owned(), count))
            .collect();
    assert_eq!(programs, expected_programs);
}
