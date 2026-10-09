//! Synthetic suite of `supercom-bp-import`: no archive and no map is opened;
//! the install paths named here do not exist. Against the skeleton every
//! test fails on a function returning `Err(Error::Unimplemented)`.

use std::path::Path;

use supercom_bp_import::{Error, cache_key, import, load};

/// An install directory that does not exist.
const ABSENT: &str = "/nonexistent/supercom-install";

/// Pins the error path of `import`: an install that is not there fails with
/// `Error::Archive` naming the archive path, not with a panic or an empty
/// catalog.
#[test]
fn import_of_an_absent_install_is_an_archive_error() {
    let result = import(Path::new(ABSENT));
    assert!(
        matches!(&result, Err(Error::Archive { path, .. }) if path.contains("units.scd")),
        "expected Archive naming units.scd, got {:?}",
        result.as_ref().err()
    );
}

/// Pins the error path of `cache_key`: the key of an absent install is an
/// `Archive` error, so a key never stands for nothing.
#[test]
fn cache_key_of_an_absent_install_is_an_archive_error() {
    let result = cache_key(Path::new(ABSENT));
    assert!(
        matches!(result, Err(Error::Archive { .. })),
        "expected Archive, got {result:?}"
    );
}

/// Pins the miss path of `load`: with nothing cached and no install, `load`
/// falls through to the import and returns its `Archive` error, and writes
/// no cache file.
#[test]
fn load_without_a_cache_or_install_fails_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let result = load(Path::new(ABSENT), dir.path());
    assert!(
        matches!(result, Err(Error::Archive { .. })),
        "expected Archive, got {result:?}"
    );
    let written = std::fs::read_dir(dir.path()).map(Iterator::count);
    assert_eq!(written.ok(), Some(0), "a failed import must leave no cache");
}

/// Pins the unwritable-cache path: a cache directory that is a regular file
/// is a `Cache` error (or an earlier `Archive` one for the absent install),
/// never `Unimplemented`.
#[test]
fn load_into_a_file_instead_of_a_directory_is_not_unimplemented() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let file = dir.path().join("not-a-directory");
    std::fs::write(&file, b"x").expect("scratch file");
    let result = load(Path::new(ABSENT), &file);
    assert!(
        matches!(result, Err(Error::Cache { .. } | Error::Archive { .. })),
        "expected Cache or Archive, got {result:?}"
    );
}
