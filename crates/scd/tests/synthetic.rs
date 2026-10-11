//! Hermetic behaviour of `supercom-scd` over archives synthesised in a
//! temporary directory. No game is needed, and no fixture file is committed:
//! each test writes tiny ZIPs built from literals.
//!
//! These pin what the installed game cannot: mount order by byte comparison,
//! precedence between archives, and the error variants.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::HashSet;
use std::error::Error as _;
use std::fs;
use std::path::{Path, PathBuf};

use supercom_scd::{Archive, Error, Vfs};

const STORED: u16 = 0;
const DEFLATED: u16 = 8;

/// A member to place in a synthetic ZIP: name, compression method, and
/// content. A `DEFLATED` member is written as one stored deflate block, so
/// it is valid deflate for any reader yet still carries method 8.
type Member<'a> = (&'a str, u16, &'a [u8]);

fn crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// `data` as a complete deflate stream made of a single stored block.
fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let len = u16::try_from(data.len()).unwrap_or(u16::MAX);
    let mut out = vec![0x01];
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    out
}

fn le16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn le32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn len32(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// The bytes of a ZIP file holding `members`.
fn zip(members: &[Member<'_>]) -> Vec<u8> {
    let mut body = Vec::new();
    let mut directory = Vec::new();
    for &(name, method, data) in members {
        let offset = len32(body.len());
        let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX);
        let payload = if method == DEFLATED {
            deflate_stored(data)
        } else {
            data.to_vec()
        };
        let (crc, size, packed) = (crc32(data), len32(data.len()), len32(payload.len()));

        le32(&mut body, 0x0403_4b50); // local file header
        le16(&mut body, 20);
        le16(&mut body, 0);
        le16(&mut body, method);
        le16(&mut body, 0);
        le16(&mut body, 0x21);
        le32(&mut body, crc);
        le32(&mut body, packed);
        le32(&mut body, size);
        le16(&mut body, name_len);
        le16(&mut body, 0);
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(&payload);

        le32(&mut directory, 0x0201_4b50); // central directory header
        le16(&mut directory, 20);
        le16(&mut directory, 20);
        le16(&mut directory, 0);
        le16(&mut directory, method);
        le16(&mut directory, 0);
        le16(&mut directory, 0x21);
        le32(&mut directory, crc);
        le32(&mut directory, packed);
        le32(&mut directory, size);
        le16(&mut directory, name_len);
        le16(&mut directory, 0);
        le16(&mut directory, 0);
        le16(&mut directory, 0);
        le16(&mut directory, 0);
        le32(&mut directory, 0);
        le32(&mut directory, offset);
        directory.extend_from_slice(name.as_bytes());
    }
    let count = u16::try_from(members.len()).unwrap_or(u16::MAX);
    let mut out = body;
    let directory_offset = len32(out.len());
    let directory_len = len32(directory.len());
    out.extend_from_slice(&directory);
    le32(&mut out, 0x0605_4b50); // end of central directory
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, count);
    le16(&mut out, count);
    le32(&mut out, directory_len);
    le32(&mut out, directory_offset);
    le16(&mut out, 0);
    out
}

/// A throwaway install directory, removed on drop.
struct Install(PathBuf);

impl Install {
    fn new(test: &str) -> Self {
        let root = std::env::temp_dir().join(format!("supercom-scd-{}-{test}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("gamedata")).expect("create the gamedata directory");
        Self(root)
    }

    fn archive(&self, file: &str, members: &[Member<'_>]) -> PathBuf {
        let path = self.0.join("gamedata").join(file);
        fs::write(&path, zip(members)).expect("write a synthetic archive");
        path
    }

    fn root(&self) -> &Path {
        &self.0
    }
}

impl Drop for Install {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn opened(path: &Path) -> Archive {
    let archive = Archive::open(path);
    assert!(
        archive.is_ok(),
        "Archive::open must succeed, got {:?}",
        archive.as_ref().err()
    );
    archive.expect("opened: asserted above")
}

fn mounted(install: &Install) -> Vfs {
    let vfs = Vfs::from_install(install.root());
    assert!(
        vfs.is_ok(),
        "Vfs::from_install must mount, got {:?}",
        vfs.as_ref().err()
    );
    vfs.expect("mounted: asserted above")
}

/// Pins `Archive::entries`: each stored member is listed once, with the
/// name in the case the archive stores and its size in bytes.
#[test]
fn archive_lists_members_with_stored_names_and_sizes() {
    let install = Install::new("lists");
    let path = install.archive(
        "one.scd",
        &[
            ("Dir/Alpha.txt", STORED, b"alpha"),
            ("beta.txt", STORED, b"bb"),
        ],
    );
    let archive = opened(&path);
    let mut listed: Vec<(String, u64)> = archive
        .entries()
        .map(|entry| (entry.name.clone(), entry.size))
        .collect();
    listed.sort();
    assert_eq!(
        listed,
        [
            ("Dir/Alpha.txt".to_string(), 5),
            ("beta.txt".to_string(), 2)
        ]
    );
}

/// Pins `Archive::read`: a member reads back its exact bytes, and the name
/// lookup ignores case.
#[test]
fn archive_read_returns_the_bytes_and_ignores_case() {
    let install = Install::new("reads");
    let path = install.archive("one.scd", &[("Dir/Alpha.txt", STORED, b"alpha")]);
    let archive = opened(&path);
    assert_eq!(archive.read("Dir/Alpha.txt").ok(), Some(b"alpha".to_vec()));
    assert_eq!(archive.read("dir/alpha.TXT").ok(), Some(b"alpha".to_vec()));
}

/// Pins `Error::NotFound` from `Archive::read`: a name the archive lacks.
/// The control arm shows a present name reading fine.
#[test]
fn archive_read_of_a_missing_name_is_not_found() {
    let install = Install::new("missing");
    let path = install.archive("one.scd", &[("here.txt", STORED, b"x")]);
    let archive = opened(&path);
    assert!(
        archive.read("here.txt").is_ok(),
        "control arm: a present name reads"
    );
    let missing = archive.read("not-here.txt");
    assert!(
        matches!(missing, Err(Error::NotFound)),
        "expected NotFound, got {:?}",
        missing.as_ref().err()
    );
}

/// Pins `Error::NotAnArchive`: bytes that are not a ZIP are rejected as such,
/// not as an I/O failure.
#[test]
fn archive_open_rejects_bytes_that_are_not_a_zip() {
    let install = Install::new("not-zip");
    let path = install.root().join("gamedata").join("junk.scd");
    fs::write(&path, b"this is not a zip archive, only plain text").expect("write junk");
    let result = Archive::open(&path);
    assert!(
        matches!(result, Err(Error::NotAnArchive)),
        "expected NotAnArchive, got {:?}",
        result.as_ref().err()
    );
}

/// Pins `Error::Io`: a file that does not exist is an I/O failure.
#[test]
fn archive_open_of_a_missing_file_is_an_io_error() {
    let install = Install::new("no-file");
    let result = Archive::open(install.root().join("gamedata").join("absent.scd"));
    assert!(
        matches!(result, Err(Error::Io(_))),
        "expected Io, got {:?}",
        result.as_ref().err()
    );
}

/// Pins `Error::Compressed`: reading a deflated member reports its method
/// number, while the archive still opens and its stored sibling still reads.
#[test]
fn archive_read_reports_the_compression_method() {
    let install = Install::new("compressed");
    let path = install.archive(
        "mixed.scd",
        &[
            ("stored.txt", STORED, b"plain"),
            ("packed.txt", DEFLATED, b"packed content"),
        ],
    );
    let archive = opened(&path);
    assert_eq!(archive.read("stored.txt").ok(), Some(b"plain".to_vec()));
    let packed = archive.read("packed.txt");
    assert!(
        matches!(packed, Err(Error::Compressed { method: 8 })),
        "expected Compressed {{ method: 8 }}, got {:?}",
        packed.as_ref().err()
    );
}

/// Pins mount order and precedence: archives mount in ascending byte order of
/// file name, so `B.scd` (0x42) precedes `a.scd` (0x61) and wins a shared
/// path, which a case-folding or directory-order mount would get wrong.
#[test]
fn vfs_mounts_in_byte_order_and_the_first_mounted_wins() {
    let install = Install::new("order");
    install.archive(
        "a.scd",
        &[
            ("shared.txt", STORED, b"from-a-longer"),
            ("only-a.txt", STORED, b"a"),
        ],
    );
    install.archive("B.scd", &[("shared.txt", STORED, b"from-B")]);
    install.archive(
        "c.scd",
        &[
            ("shared.txt", STORED, b"from-c"),
            ("only-c.txt", STORED, b"c"),
        ],
    );
    let vfs = mounted(&install);
    assert_eq!(vfs.read("shared.txt").ok(), Some(b"from-B".to_vec()));
    assert_eq!(
        vfs.read("only-a.txt").ok(),
        Some(b"a".to_vec()),
        "later archives still contribute"
    );
    assert_eq!(
        vfs.read("only-c.txt").ok(),
        Some(b"c".to_vec()),
        "later archives still contribute"
    );
}

/// Pins `Vfs::files`: each visible path is enumerated once, as the winning
/// copy (its size, not a shadowed copy's), with paths compared ignoring case.
#[test]
fn vfs_enumerates_each_visible_path_once_as_the_winning_copy() {
    let install = Install::new("files");
    install.archive("B.scd", &[("Shared.txt", STORED, b"from-B")]);
    install.archive(
        "a.scd",
        &[
            ("shared.TXT", STORED, b"from-a-longer"),
            ("only-a.txt", STORED, b"a"),
        ],
    );
    let vfs = mounted(&install);
    let mut listed: Vec<(String, u64)> = vfs
        .files()
        .map(|entry| (entry.name.to_lowercase(), entry.size))
        .collect();
    listed.sort();
    assert_eq!(
        listed,
        [("only-a.txt".to_string(), 1), ("shared.txt".to_string(), 6)]
    );
    let names: HashSet<String> = vfs.files().map(|entry| entry.name.to_lowercase()).collect();
    assert_eq!(names.len(), 2, "no path enumerated twice");
}

/// Pins `Vfs::from_install`: files in `gamedata` that are not `.scd`
/// archives are ignored, and lookups through the `Vfs` ignore case.
#[test]
fn vfs_ignores_non_archives_and_looks_up_ignoring_case() {
    let install = Install::new("ignores");
    install.archive("one.scd", &[("Data/File.TXT", STORED, b"payload")]);
    fs::write(
        install.root().join("gamedata").join("notes.txt"),
        b"not an archive",
    )
    .expect("write notes");
    let vfs = mounted(&install);
    assert_eq!(vfs.read("data/file.txt").ok(), Some(b"payload".to_vec()));
    assert_eq!(vfs.files().count(), 1, "only the archive's file is visible");
}

/// Pins `Error::NotFound` from `Vfs::read`: a path no archive holds. The
/// control arm shows a held path reading fine.
#[test]
fn vfs_read_of_a_missing_path_is_not_found() {
    let install = Install::new("vfs-missing");
    install.archive("one.scd", &[("held.txt", STORED, b"x")]);
    let vfs = mounted(&install);
    assert!(
        vfs.read("held.txt").is_ok(),
        "control arm: a held path reads"
    );
    let missing = vfs.read("absent.txt");
    assert!(
        matches!(missing, Err(Error::NotFound)),
        "expected NotFound, got {:?}",
        missing.as_ref().err()
    );
}

/// Pins `Error::Compressed` through the `Vfs`: a deflated member surfaces
/// its method number rather than garbage, and the install still mounts.
#[test]
fn vfs_read_reports_the_compression_method() {
    let install = Install::new("vfs-compressed");
    install.archive(
        "mixed.scd",
        &[
            ("stored.txt", STORED, b"plain"),
            ("packed.txt", DEFLATED, b"packed content"),
        ],
    );
    let vfs = mounted(&install);
    assert_eq!(vfs.read("stored.txt").ok(), Some(b"plain".to_vec()));
    let packed = vfs.read("packed.txt");
    assert!(
        matches!(packed, Err(Error::Compressed { method: 8 })),
        "expected Compressed {{ method: 8 }}, got {:?}",
        packed.as_ref().err()
    );
}

/// Pins the `Display` contract of `Error`: every variant renders a distinct,
/// non-empty message, and `Compressed` names its method.
#[test]
fn error_display_is_distinct_and_names_the_compression_method() {
    let errors = [
        Error::Io(std::io::Error::other("disk on fire")),
        Error::NotAnArchive,
        Error::Compressed { method: 8 },
        Error::NotFound,
        Error::Unimplemented,
    ];
    let messages: Vec<String> = errors.iter().map(ToString::to_string).collect();
    assert!(
        messages.iter().all(|message| !message.is_empty()),
        "empty message in {messages:?}"
    );
    let distinct: HashSet<&String> = messages.iter().collect();
    assert_eq!(
        distinct.len(),
        errors.len(),
        "messages must differ: {messages:?}"
    );
    assert!(
        messages[2].contains('8'),
        "Compressed must name its method: {:?}",
        messages[2]
    );
}

/// Pins the `std::error::Error` contract of `Error::Io`: the wrapped I/O
/// error is the `source()`, and the other variants have none.
#[test]
fn error_io_exposes_its_source() {
    let io = Error::Io(std::io::Error::other("disk on fire"));
    assert!(
        io.source()
            .is_some_and(|source| source.to_string() == "disk on fire")
    );
    assert!(Error::NotFound.source().is_none());
}
