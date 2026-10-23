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

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use supercom_dds::Header;
use supercom_scd::Vfs;
use supercom_scmap::Map;

/// `.scmap` files in the install.
const MAPS: usize = 60;

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
