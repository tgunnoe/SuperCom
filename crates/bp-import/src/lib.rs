//! Importer of the installed game's blueprints into a typed catalog.
//!
//! [`import`] parses and validates every unit, projectile, and prop
//! blueprint in an install; [`load`] fronts it with a cache keyed by
//! [`cache_key`]. The load budget is 10 s cold and 1 s cached; the
//! `bench_import` binary measures it.
//!
//! This is the skeleton: every function returns [`Error::Unimplemented`].

use std::fmt;
use std::path::Path;

use supercom_data::{Projectile, Prop, Unit};

/// Every v0 blueprint of an install.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    /// The unit blueprints of `units.scd`.
    pub units: Vec<Unit>,
    /// The projectile blueprints of every archive that holds them.
    pub projectiles: Vec<Projectile>,
    /// The prop blueprints of every archive that holds them.
    pub props: Vec<Prop>,
}

/// Why an import or a cache operation failed.
#[derive(Debug)]
pub enum Error {
    /// An archive could not be opened or read.
    Archive {
        /// The archive's path.
        path: String,
        /// The archive error.
        source: supercom_scd::Error,
    },
    /// A member did not parse.
    Parse {
        /// The member's path inside its archive.
        member: String,
        /// The parser error.
        source: supercom_bp::Error,
    },
    /// A member parsed but did not fit the model.
    Model {
        /// The member's path inside its archive.
        member: String,
        /// The model error.
        source: supercom_data::Error,
    },
    /// The cache could not be read or written.
    Cache {
        /// The cache file's path.
        path: String,
        /// What went wrong.
        reason: String,
    },
    /// Reserved for the skeleton: the operation is not built yet.
    Unimplemented,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Archive { path, source } => write!(f, "archive {path}: {source}"),
            Self::Parse { member, source } => write!(f, "parsing {member}: {source}"),
            Self::Model { member, source } => write!(f, "modelling {member}: {source}"),
            Self::Cache { path, reason } => write!(f, "cache {path}: {reason}"),
            Self::Unimplemented => f.write_str("not implemented"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Archive { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Model { source, .. } => Some(source),
            Self::Cache { .. } | Self::Unimplemented => None,
        }
    }
}

/// Parses and validates every unit, projectile, and prop blueprint in the
/// install at `install`.
///
/// # Errors
///
/// The first archive, parse, or model error.
pub fn import(install: &Path) -> Result<Catalog, Error> {
    let _ = install;
    Err(Error::Unimplemented)
}

/// Returns the cached catalog under `cache_dir` when its key matches the
/// install, else imports and writes the cache.
///
/// # Errors
///
/// As [`import`], and [`Error::Cache`] if the cache cannot be written.
pub fn load(install: &Path, cache_dir: &Path) -> Result<Catalog, Error> {
    let _ = (install, cache_dir);
    Err(Error::Unimplemented)
}

/// The cache key of the install: it changes when the archives the import
/// reads change.
///
/// # Errors
///
/// [`Error::Archive`] if an archive cannot be read.
pub fn cache_key(install: &Path) -> Result<String, Error> {
    let _ = install;
    Err(Error::Unimplemented)
}
