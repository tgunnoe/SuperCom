//! Reader for `.scd` archives and the virtual filesystem mounted over an
//! installed game's `gamedata/*.scd`.
//!
//! An `.scd` file is a ZIP archive. The installed game stores every member
//! uncompressed (ZIP method 0), which is the only method this crate reads.
//!
//! Opening an archive reads its member directory and each member's local
//! header, and no member data. A member's bytes are read when asked for, by
//! reopening the file, so neither an [`Archive`] nor a [`Vfs`] holds an open
//! file.

use std::collections::{HashMap, hash_map};
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use zip::result::ZipError;
use zip::{CompressionMethod, ZipArchive};

/// ZIP compression method id of a member stored without compression.
const STORED: u16 = 0;

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
    /// The file system failed: a missing file, a short read. The wrapped
    /// error's message names the archive, and the member when one was being
    /// read.
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
    /// Reserved: no operation of this crate returns it any longer. It stays
    /// so that the public API is unchanged.
    Unimplemented,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(f, "i/o error: {source}"),
            Self::NotAnArchive => f.write_str("not a ZIP archive"),
            Self::Compressed { method } => write!(
                f,
                "member is compressed with ZIP method {method}; \
                 only stored members (method 0) are supported"
            ),
            Self::NotFound => f.write_str("no such file"),
            Self::Unimplemented => f.write_str("not implemented"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::NotAnArchive | Self::Compressed { .. } | Self::NotFound | Self::Unimplemented => {
                None
            }
        }
    }
}

/// An I/O failure with a note of what was being done. It rides as the payload
/// of the [`io::Error`] inside [`Error::Io`], so that the message names the
/// archive or member without a new variant on the public enum. The original
/// error's text is part of this one's, so it is not chained as a source too.
#[derive(Debug)]
struct Context {
    doing: String,
    source: io::Error,
}

impl fmt::Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.doing, self.source)
    }
}

impl std::error::Error for Context {}

/// Wraps an I/O failure, keeping its kind, with what was being done.
fn io_error(source: io::Error, doing: fmt::Arguments<'_>) -> Error {
    let kind = source.kind();
    let doing = doing.to_string();
    Error::Io(io::Error::new(kind, Context { doing, source }))
}

/// Wraps a failure of the `zip` crate as an I/O failure with what was being
/// done: a real I/O error keeps its kind, anything else is bad data.
fn zip_error(source: ZipError, doing: fmt::Arguments<'_>) -> Error {
    let source = match source {
        ZipError::Io(source) => source,
        other => io::Error::new(io::ErrorKind::InvalidData, other.to_string()),
    };
    io_error(source, doing)
}

/// Folds a member name or path for the case-insensitive comparisons.
fn fold(name: &str) -> String {
    name.to_lowercase()
}

/// The ZIP method id of a member. `zip` gives the number only through its
/// deprecated `to_u16`.
#[allow(
    deprecated,
    reason = "zip 7 exposes a member's numeric method id only through to_u16"
)]
fn method_id(method: CompressionMethod) -> u16 {
    method.to_u16()
}

/// Where a member's bytes lie in its archive file.
#[derive(Clone, Copy, Debug)]
struct Location {
    /// Offset of the member's first data byte.
    data_start: u64,
    /// Bytes the member occupies in the archive.
    stored_len: u64,
    /// ZIP compression method id.
    method: u16,
}

/// A file member: what is listed, and where to find it.
#[derive(Debug)]
struct Member {
    entry: Entry,
    at: Location,
}

/// Reads `len` bytes at offset `start` of the file at `path`. The range is
/// checked against the file's length before anything is allocated, so a
/// directory that lies about a size cannot ask for more memory than the file
/// holds.
fn read_range(path: &Path, start: u64, len: u64) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    if start.checked_add(len).is_none_or(|end| end > file_len) {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "the member extends past the end of the archive file",
        ));
    }
    let len = usize::try_from(len).map_err(|_| io::ErrorKind::OutOfMemory)?;
    let mut bytes = vec![0; len];
    file.seek(SeekFrom::Start(start))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// A single `.scd` archive.
#[derive(Debug)]
pub struct Archive {
    path: PathBuf,
    /// File members in the order of the archive's directory.
    members: Vec<Member>,
    /// Index into `members` by folded name; the first of equal names wins.
    by_name: HashMap<String, usize>,
}

impl Archive {
    /// Opens the archive at `path` and reads its member directory.
    ///
    /// Directory members (names ending in `/`) are not listed. Members whose
    /// names are identical collapse into one, the last of them; names that
    /// differ only in case are kept apart, and lookups find the first.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the file cannot be read; [`Error::NotAnArchive`] if it
    /// is not a ZIP archive.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|source| {
            io_error(source, format_args!("opening archive {}", path.display()))
        })?;
        let mut zip = ZipArchive::new(file).map_err(|source| match source {
            ZipError::Io(source) => io_error(
                source,
                format_args!("reading the member directory of {}", path.display()),
            ),
            _ => Error::NotAnArchive,
        })?;

        let count = zip.len();
        let mut members = Vec::with_capacity(count);
        let mut by_name = HashMap::with_capacity(count);
        for index in 0..count {
            let name = zip.name_for_index(index).unwrap_or_default().to_owned();
            if name.ends_with('/') {
                continue;
            }
            let member = zip.by_index_raw(index).map_err(|source| {
                zip_error(
                    source,
                    format_args!("reading the header of {name} in {}", path.display()),
                )
            })?;
            let at = Location {
                data_start: member.data_start(),
                stored_len: member.compressed_size(),
                method: method_id(member.compression()),
            };
            let entry = Entry {
                name,
                size: member.size(),
            };
            by_name.entry(fold(&entry.name)).or_insert(members.len());
            members.push(Member { entry, at });
        }
        Ok(Self {
            path: path.to_path_buf(),
            members,
            by_name,
        })
    }

    /// The archive's file members, borrowed from the archive.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.members.iter().map(|member| &member.entry)
    }

    /// Reads the member called `name`; the lookup ignores case.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no member has that name;
    /// [`Error::Compressed`] if the member is not stored; [`Error::Io`] if
    /// the read fails.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, Error> {
        let member = self
            .by_name
            .get(&fold(name))
            .and_then(|&index| self.members.get(index))
            .ok_or(Error::NotFound)?;
        if member.at.method != STORED {
            return Err(Error::Compressed {
                method: member.at.method,
            });
        }
        let failed = |source| {
            io_error(
                source,
                format_args!("reading {} from {}", member.entry.name, self.path.display()),
            )
        };
        if member.at.stored_len != member.entry.size {
            return Err(failed(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "stored length {} differs from the listed size {}",
                    member.at.stored_len, member.entry.size
                ),
            )));
        }
        read_range(&self.path, member.at.data_start, member.at.stored_len).map_err(failed)
    }
}

/// The installed game's archives mounted as one namespace.
#[derive(Debug)]
pub struct Vfs {
    /// The mounted archives, in mount order.
    archives: Vec<Archive>,
    /// The visible files: each path once, as its winning copy.
    files: Vec<Entry>,
    /// Index into `archives` of the archive that supplies each folded path.
    owners: HashMap<String, usize>,
}

/// The bytes of a path's file name, for ordering archives by plain byte
/// comparison.
fn file_name_bytes(path: &Path) -> &[u8] {
    path.file_name()
        .map(OsStr::as_encoded_bytes)
        .unwrap_or_default()
}

/// Whether `path` is a file, following symlinks.
fn is_file(path: &Path) -> Result<bool, Error> {
    fs::metadata(path)
        .map(|meta| meta.is_file())
        .map_err(|source| io_error(source, format_args!("inspecting {}", path.display())))
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
    pub fn from_install(dir: impl AsRef<Path>) -> Result<Self, Error> {
        let gamedata = dir.as_ref().join("gamedata");
        let listing_failed =
            |source| io_error(source, format_args!("listing {}", gamedata.display()));
        let mut paths = Vec::new();
        for item in fs::read_dir(&gamedata).map_err(listing_failed)? {
            let path = item.map_err(listing_failed)?.path();
            if path.extension() == Some(OsStr::new("scd")) && is_file(&path)? {
                paths.push(path);
            }
        }
        paths.sort_by(|a, b| file_name_bytes(a).cmp(file_name_bytes(b)));

        let mut archives = Vec::with_capacity(paths.len());
        let mut files = Vec::new();
        let mut owners = HashMap::new();
        for path in &paths {
            let archive = Archive::open(path)?;
            for entry in archive.entries() {
                if let hash_map::Entry::Vacant(slot) = owners.entry(fold(&entry.name)) {
                    slot.insert(archives.len());
                    files.push(entry.clone());
                }
            }
            archives.push(archive);
        }
        Ok(Self {
            archives,
            files,
            owners,
        })
    }

    /// Every visible file, each path once: archives in mount order, and each
    /// archive's members in the order of its directory.
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
    pub fn read(&self, path: &str) -> Result<Vec<u8>, Error> {
        self.owners
            .get(&fold(path))
            .and_then(|&index| self.archives.get(index))
            .ok_or(Error::NotFound)?
            .read(path)
    }
}
