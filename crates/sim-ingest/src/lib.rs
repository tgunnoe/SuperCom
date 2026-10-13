//! Builds the simulation's static world from the installed game.
//!
//! `supercom-sim` forbids the filesystem, so every file the world is built
//! from is read here: the blueprint catalog through `supercom-bp-import`, the
//! map's `.scmap` and `_save.lua` through `supercom-scmap` and `supercom-bp`.
//! The parsed values are converted into the sim's types and handed to
//! `World::from_parts`, which computes both digests (SIM-049 to SIM-051).

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use supercom_sim::World;

/// Why the world could not be built.
#[derive(Debug)]
pub enum Error {
    /// The importer or its cache failed.
    Catalog(supercom_bp_import::Error),
    /// A map file could not be read.
    Io {
        path: String,
        source: std::io::Error,
    },
    /// The map folder lacks a file of the given suffix.
    MissingMapFile { folder: String, suffix: String },
    /// The `.scmap` did not parse.
    Map(supercom_scmap::Error),
    /// The save file did not parse.
    Save(supercom_bp::Error),
    /// The sim refused the parsed values.
    Sim(supercom_sim::Error),
    /// Stub: the loader is not implemented yet.
    Unimplemented,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(e) => write!(f, "cannot import the catalog: {e}"),
            Self::Io { path, source } => write!(f, "cannot read {path}: {source}"),
            Self::MissingMapFile { folder, suffix } => {
                write!(f, "map folder {folder} has no file ending {suffix}")
            }
            Self::Map(e) => write!(f, "cannot parse the map: {e}"),
            Self::Save(e) => write!(f, "cannot parse the save file: {e}"),
            Self::Sim(e) => write!(f, "the sim refused the world: {e}"),
            Self::Unimplemented => f.write_str("the loader is not implemented"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Catalog(e) => Some(e),
            Self::Io { source, .. } => Some(source),
            Self::Map(e) => Some(e),
            Self::Save(e) => Some(e),
            Self::Sim(e) => Some(e),
            Self::MissingMapFile { .. } | Self::Unimplemented => None,
        }
    }
}

/// Build the world for `map_folder` (a folder name under `<install>/maps`)
/// from the installed catalog, caching the catalog under `cache_dir`. The
/// folder's `.scmap` and `_save.lua` are matched case-insensitively. Stub.
///
/// # Errors
///
/// Any [`Error`] variant: a failed import, an unreadable or missing map
/// file, a parse failure, or a value the sim refuses.
pub fn load_world(install: &Path, cache_dir: &Path, map_folder: &str) -> Result<Arc<World>, Error> {
    let _ = (install, cache_dir, map_folder);
    Err(Error::Unimplemented)
}
