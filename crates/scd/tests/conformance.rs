//! Conformance of `supercom-scd` against the installed game.
//!
//! Every test here is game-dependent: it begins by asking `supercom_testkit`
//! for the install and returns (skips, loudly) when there is none. Run them
//! against the real game with `SUPERCOM_REQUIRE_GAME=1`.
//!
//! The expected numbers are measured on the installed Forged Alliance:
//! 17 archives, 23,015 file entries across them, of which 4 `.lua` paths
//! exist in both `lua.scd` and `mohodata.scd`, leaving 23,011 distinct paths.
//! No game content is stored here: names, counts, sizes, and a text prefix
//! only.
//!
//! Each test's first assertion is on the behaviour under test, never on the
//! environment, so a failure points at the capability and not at the install.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use supercom_scd::{Archive, Entry, Error, Vfs};

/// The commander's blueprint: present once, in `units.scd`.
const COMMANDER_BLUEPRINT: &str = "units/UEL0001/UEL0001_unit.bp";

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

/// Opens one archive, failing the calling test with the error if it cannot.
fn opened(path: &Path) -> Archive {
    let archive = Archive::open(path);
    assert!(
        archive.is_ok(),
        "Archive::open({}) must succeed, got {:?}",
        path.display(),
        archive.as_ref().err()
    );
    archive.expect("opened: asserted above")
}

/// The install's `gamedata/*.scd` files in ascending byte order of file name,
/// found without the code under test.
fn archive_paths(fa: &Path) -> Vec<PathBuf> {
    let listing = fs::read_dir(fa.join("gamedata"));
    assert!(
        listing.is_ok(),
        "cannot list gamedata: {:?}",
        listing.as_ref().err()
    );
    let mut paths: Vec<PathBuf> = listing
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "scd"))
        .collect();
    paths.sort_by_key(|path| path.file_name().map(std::ffi::OsStr::to_os_string));
    paths
}

/// An archive's members that are files (ZIP directory members end in `/`).
fn file_entries(archive: &Archive) -> impl Iterator<Item = &Entry> {
    archive.entries().filter(|entry| !entry.name.ends_with('/'))
}

/// Pins `Vfs::from_install`: all 17 `gamedata/*.scd` archives are mounted,
/// witnessed by every archive that holds a file nobody else holds being
/// readable through the `Vfs` with that archive's own bytes. The 3 archives
/// that hold no files (`ambience`, `moholua`, `skins`) have no witness.
#[test]
fn conformance_install_mounts_every_gamedata_archive() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let paths = archive_paths(&fa);
    assert_eq!(paths.len(), 17, "the install holds 17 gamedata archives");
    let archives: Vec<Archive> = paths.iter().map(|path| opened(path)).collect();

    let mut holders: HashMap<String, usize> = HashMap::new();
    for archive in &archives {
        for entry in file_entries(archive) {
            *holders.entry(entry.name.to_lowercase()).or_default() += 1;
        }
    }
    let mut witnessed = 0;
    for (path, archive) in paths.iter().zip(&archives) {
        let own = file_entries(archive).find(|entry| holders[&entry.name.to_lowercase()] == 1);
        let Some(own) = own else { continue };
        witnessed += 1;
        assert_eq!(
            vfs.read(&own.name).ok(),
            archive.read(&own.name).ok(),
            "{} is not mounted: its own file {} is not served",
            path.display(),
            own.name
        );
    }
    assert_eq!(witnessed, 14, "14 of the 17 archives hold files");
}

/// Pins `Vfs::files`: it enumerates each visible path once. The archives
/// hold 23,015 file entries between them (`Archive::entries`, files only);
/// 4 paths are held twice, so first-mounted-wins leaves 23,011 distinct
/// paths, and the shadowed copies are not enumerated.
#[test]
fn conformance_vfs_enumerates_each_visible_file_once() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let visible = vfs.files().count();
    assert_eq!(
        visible, 23_011,
        "23,015 file entries minus 4 shadowed copies"
    );
    let distinct: HashSet<String> = vfs.files().map(|entry| entry.name.to_lowercase()).collect();
    assert_eq!(distinct.len(), visible, "no path may be enumerated twice");

    let raw: usize = archive_paths(&fa)
        .iter()
        .map(|path| file_entries(&opened(path)).count())
        .sum();
    assert_eq!(raw, 23_015, "file entries across the 17 archives");
}

/// Pins `Vfs::read` over stored data: every 100th enumerated file (at least
/// 200 files, in enumeration order) reads without a `Compressed` error, and
/// the bytes read match the size the entry reports.
#[test]
fn conformance_sampled_files_read_stored_with_their_listed_size() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let sample: Vec<&Entry> = vfs.files().step_by(100).collect();
    assert!(
        sample.len() >= 200,
        "sampled {} files, need 200",
        sample.len()
    );
    for entry in sample {
        let read = vfs.read(&entry.name);
        assert!(
            !matches!(read, Err(Error::Compressed { .. })),
            "{} reported as compressed: {:?}",
            entry.name,
            read.as_ref().err()
        );
        assert!(
            read.is_ok(),
            "{} unreadable: {:?}",
            entry.name,
            read.as_ref().err()
        );
        let len = read.map(|bytes| bytes.len() as u64).ok();
        assert_eq!(
            len,
            Some(entry.size),
            "{} read a different size than listed",
            entry.name
        );
    }
}

/// Pins `Vfs::read` on a known file: the commander's blueprint reads to more
/// than 1,000 bytes and starts with the text `UnitBlueprint`.
#[test]
fn conformance_reads_the_commander_blueprint() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let read = vfs.read(COMMANDER_BLUEPRINT);
    assert!(
        read.is_ok(),
        "{COMMANDER_BLUEPRINT} unreadable: {:?}",
        read.as_ref().err()
    );
    let bytes = read.unwrap_or_default();
    assert!(bytes.len() > 1000, "blueprint is {} bytes", bytes.len());
    assert!(
        bytes.starts_with(b"UnitBlueprint"),
        "blueprint does not start with UnitBlueprint"
    );
}

/// Pins case-insensitive lookup: a lower-cased (and an upper-cased) path
/// resolves to the same file as the stored spelling, through the `Vfs` and
/// through the `Archive` that holds it.
#[test]
fn conformance_lookup_ignores_case() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let stored = vfs.read(COMMANDER_BLUEPRINT);
    assert!(
        stored.is_ok(),
        "{COMMANDER_BLUEPRINT} unreadable: {:?}",
        stored.as_ref().err()
    );
    let stored = stored.ok();
    let lower = COMMANDER_BLUEPRINT.to_lowercase();
    let upper = COMMANDER_BLUEPRINT.to_uppercase();
    assert_eq!(vfs.read(&lower).ok(), stored, "lower-cased path");
    assert_eq!(vfs.read(&upper).ok(), stored, "upper-cased path");

    let units = opened(&fa.join("gamedata").join("units.scd"));
    assert_eq!(
        units.read(&lower).ok(),
        stored,
        "lower-cased path in the archive"
    );
}

/// Pins `Error::NotFound`: a path no archive holds is `NotFound` from the
/// `Vfs`, and a name the archive lacks is `NotFound` from the `Archive`.
#[test]
fn conformance_missing_path_is_not_found() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let missing = vfs.read("units/NOPE0000/NOPE0000_unit.bp");
    assert!(
        matches!(missing, Err(Error::NotFound)),
        "expected NotFound from the Vfs, got {:?}",
        missing.as_ref().err()
    );
    let units = opened(&fa.join("gamedata").join("units.scd"));
    let missing = units.read("units/NOPE0000/NOPE0000_unit.bp");
    assert!(
        matches!(missing, Err(Error::NotFound)),
        "expected NotFound from the Archive, got {:?}",
        missing.as_ref().err()
    );
}

/// Pins first-mounted-wins precedence on real data: the 4 paths held by both
/// `lua.scd` and `mohodata.scd` (different bytes in each) are served from
/// `lua.scd`, which sorts first in byte order of file names.
#[test]
fn conformance_first_mounted_archive_wins_shared_paths() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let paths = archive_paths(&fa);
    let archives: Vec<Archive> = paths.iter().map(|path| opened(path)).collect();
    let mut holders: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, archive) in archives.iter().enumerate() {
        for entry in file_entries(archive) {
            holders
                .entry(entry.name.to_lowercase())
                .or_default()
                .push(index);
        }
    }
    let shared: Vec<(&String, &Vec<usize>)> = holders
        .iter()
        .filter(|(_, held_by)| held_by.len() > 1)
        .collect();
    assert_eq!(shared.len(), 4, "paths held by more than one archive");

    for (name, held_by) in shared {
        let first = archives[held_by[0]].read(name).ok();
        let second = archives[held_by[1]].read(name).ok();
        assert!(
            first.is_some() && second.is_some(),
            "{name} unreadable in its archives"
        );
        assert_ne!(
            first, second,
            "{name} must differ between archives to witness precedence"
        );
        assert_eq!(
            vfs.read(name).ok(),
            first,
            "{name} must come from the first-mounted archive {}",
            paths[held_by[0]].display()
        );
    }
}
