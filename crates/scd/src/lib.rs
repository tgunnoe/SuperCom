//! Reader for `.scd` archives and the virtual filesystem mounted over an
//! installed game's `gamedata/*.scd`.
//!
//! An `.scd` file is a ZIP archive. The installed game stores every member
//! uncompressed (ZIP method 0), which is the only method this crate reads.
//!
//! SKELETON: every function here is a deliberately unimplemented stub that
//! returns [`Error::Unimplemented`] or an empty value. The test suites in
//! `tests/` fail against it until it is implemented.

use std::fmt;
use std::path::Path;

/// One file inside an archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// Path inside the archive, in the case the archive stores it.
    pub name: String,
    /// Size of the file in bytes once read.
    pub size: u64,
}

/// Why an archive or a path could not be read.
#[derive(Debug)]
pub enum Error {
    /// The file system failed: a missing file, a short read.
    Io(std::io::Error),
    /// The file is not a ZIP archive.
    NotAnArchive,
    /// The member uses a compression method this crate does not read; only
    /// stored members (method 0) are supported.
    Compressed {
        /// The ZIP compression method id of the member.
        method: u16,
    },
    /// No member has the requested name.
    NotFound,
    /// The skeleton has not been implemented yet.
    Unimplemented,
}

// STUB: writes nothing. The implementation writes one informative line per
// variant, naming the method for `Compressed`.
impl fmt::Display for Error {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

// STUB: the implementation returns the inner error from `source()` for `Io`.
impl std::error::Error for Error {}

/// A single `.scd` archive.
#[derive(Debug)]
pub struct Archive {
    entries: Vec<Entry>,
}

impl Archive {
    /// Opens the archive at `path` and reads its member directory.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the file cannot be read; [`Error::NotAnArchive`] if it
    /// is not a ZIP archive.
    pub fn open(_path: impl AsRef<Path>) -> Result<Self, Error> {
        Err(Error::Unimplemented)
    }

    /// The archive's file members, borrowed from the archive.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    /// Reads the member called `name`; the lookup ignores case.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no member has that name;
    /// [`Error::Compressed`] if the member is not stored; [`Error::Io`] if
    /// the read fails.
    pub fn read(&self, _name: &str) -> Result<Vec<u8>, Error> {
        Err(Error::Unimplemented)
    }
}

/// The installed game's archives mounted as one namespace.
#[derive(Debug)]
pub struct Vfs {
    files: Vec<Entry>,
}

impl Vfs {
    /// Mounts every `gamedata/*.scd` archive under the install at `dir`.
    ///
    /// Archives are mounted in ascending byte order of their file names:
    /// plain byte comparison of the UTF-8 name, with no locale and no case
    /// folding, so `B.scd` is mounted before `a.scd`. Files that are not
    /// `.scd` archives are ignored.
    ///
    /// Precedence is first-mounted-wins: when several archives hold the same
    /// path (compared ignoring case), the earliest-mounted archive supplies
    /// the file, and the later copies are shadowed. [`Vfs::files`] and
    /// [`Vfs::read`] both see only the winning copy.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the install's `gamedata` directory cannot be listed;
    /// otherwise the first error from opening an archive.
    pub fn from_install(_dir: impl AsRef<Path>) -> Result<Self, Error> {
        Err(Error::Unimplemented)
    }

    /// Every visible file, each path once, in a deterministic order.
    pub fn files(&self) -> impl Iterator<Item = &Entry> {
        self.files.iter()
    }

    /// Reads `path` from the first-mounted archive that holds it; the lookup
    /// ignores case.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no mounted archive holds the path;
    /// [`Error::Compressed`] if the member is not stored; [`Error::Io`] if
    /// the read fails.
    pub fn read(&self, _path: &str) -> Result<Vec<u8>, Error> {
        Err(Error::Unimplemented)
    }
}
