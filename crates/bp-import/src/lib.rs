//! Importer of the installed game's blueprints into a typed catalog.
//!
//! [`import`] parses and validates every unit, projectile, and prop
//! blueprint in an install; [`load`] fronts it with a cache keyed by
//! [`cache_key`]. The load budget is 10 s cold and 1 s cached; the
//! `bench_import` binary measures it.

use std::fmt::{self, Write as _};
use std::fs;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use supercom_data::{Kind, Projectile, Prop, Unit};

/// Every v0 blueprint of an install.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Archive { path, source } => write!(f, "archive {path}: {source}"),
            Self::Parse { member, source } => write!(f, "parsing {member}: {source}"),
            Self::Model { member, source } => write!(f, "modelling {member}: {source}"),
            Self::Cache { path, reason } => write!(f, "cache {path}: {reason}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Archive { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Model { source, .. } => Some(source),
            Self::Cache { .. } => None,
        }
    }
}

/// The archives the import reads, in mount order. `units.scd` comes first
/// and is the only source of units.
const ARCHIVES: [&str; 6] = [
    "units.scd",
    "projectiles.scd",
    "effects.scd",
    "env.scd",
    "meshes.scd",
    "props.scd",
];

/// The file the catalog is cached in, under the cache directory.
const CACHE_FILE: &str = "catalog.json";

/// Bump when the cached layout or the import's reading of a member changes.
const CACHE_FORMAT: u32 = 1;

fn archive_path(install: &Path, name: &str) -> PathBuf {
    install.join("gamedata").join(name)
}

fn archive_error(path: &Path, source: supercom_scd::Error) -> Error {
    Error::Archive {
        path: path.display().to_string(),
        source,
    }
}

/// The blueprint id of a member: its file stem without the kind's suffix
/// (`_unit`, `_proj`, `_prop`), compared without regard to case.
fn blueprint_id(member: &str, suffix: &str) -> String {
    let file = member.rsplit('/').next().unwrap_or(member);
    let stem = file
        .get(..file.len().saturating_sub(".bp".len()))
        .unwrap_or(file);
    let cut = stem.len().saturating_sub(suffix.len());
    match stem.get(cut..) {
        Some(tail) if tail.eq_ignore_ascii_case(suffix) => stem.get(..cut).unwrap_or(stem),
        _ => stem,
    }
    .to_owned()
}

/// Reads, parses, validates, and projects every member of `archive` whose
/// name ends in `<suffix>.bp` (ignoring case), in path order.
fn read_kind<T>(
    archive: &supercom_scd::Archive,
    path: &Path,
    kind: Kind,
    suffix: &str,
    project: impl Fn(&str, &supercom_bp::Value) -> Result<T, supercom_data::Error>,
    out: &mut Vec<T>,
) -> Result<(), Error> {
    let ending = format!("{suffix}.bp");
    let mut members: Vec<&str> = archive
        .entries()
        .map(|e| e.name.as_str())
        .filter(|n| n.to_ascii_lowercase().ends_with(&ending))
        .collect();
    members.sort_unstable();
    for member in members {
        let bytes = archive
            .read(member)
            .map_err(|source| archive_error(path, source))?;
        let blueprint = supercom_bp::parse_bytes(&bytes).map_err(|source| Error::Parse {
            member: member.to_owned(),
            source,
        })?;
        let model = |source| Error::Model {
            member: member.to_owned(),
            source,
        };
        supercom_data::validate(kind, &blueprint.body).map_err(model)?;
        let id = blueprint_id(member, suffix);
        out.push(project(&id, &blueprint.body).map_err(model)?);
    }
    Ok(())
}

/// Parses and validates every unit, projectile, and prop blueprint in the
/// install at `install`.
///
/// Units come from `units.scd` alone; projectiles and props from every
/// archive that holds them. The catalog's order is the mount order of the
/// archives, then the member paths.
///
/// # Errors
///
/// The first archive, parse, or model error.
pub fn import(install: &Path) -> Result<Catalog, Error> {
    let mut catalog = Catalog {
        units: Vec::new(),
        projectiles: Vec::new(),
        props: Vec::new(),
    };
    for name in ARCHIVES {
        let path = archive_path(install, name);
        let archive = supercom_scd::Archive::open(&path).map_err(|e| archive_error(&path, e))?;
        if name == ARCHIVES[0] {
            read_kind(
                &archive,
                &path,
                Kind::Unit,
                "_unit",
                supercom_data::unit_from_value,
                &mut catalog.units,
            )?;
        }
        read_kind(
            &archive,
            &path,
            Kind::Projectile,
            "_proj",
            supercom_data::projectile_from_value,
            &mut catalog.projectiles,
        )?;
        read_kind(
            &archive,
            &path,
            Kind::Prop,
            "_prop",
            supercom_data::prop_from_value,
            &mut catalog.props,
        )?;
    }
    Ok(catalog)
}

/// What the cache file holds: the key it was made for and the catalog.
#[derive(Deserialize, Serialize)]
struct Cached {
    key: String,
    catalog: Catalog,
}

fn cache_error(path: &Path, reason: impl fmt::Display) -> Error {
    Error::Cache {
        path: path.display().to_string(),
        reason: reason.to_string(),
    }
}

/// Returns the cached catalog under `cache_dir` when its key matches the
/// install, else imports and writes the cache.
///
/// A cache file that cannot be read or parsed counts as a miss. The write
/// goes to a temporary name first and is renamed into place.
///
/// # Errors
///
/// As [`import`], and [`Error::Cache`] if the cache cannot be written.
pub fn load(install: &Path, cache_dir: &Path) -> Result<Catalog, Error> {
    let key = cache_key(install)?;
    let file = cache_dir.join(CACHE_FILE);
    let hit = fs::read(&file)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Cached>(&bytes).ok())
        .filter(|cached| cached.key == key);
    if let Some(cached) = hit {
        return Ok(cached.catalog);
    }
    let catalog = import(install)?;
    fs::create_dir_all(cache_dir).map_err(|e| cache_error(cache_dir, e))?;
    let cached = Cached { key, catalog };
    let bytes = serde_json::to_vec(&cached).map_err(|e| cache_error(&file, e))?;
    let staging = cache_dir.join(format!("{CACHE_FILE}.tmp"));
    fs::write(&staging, bytes).map_err(|e| cache_error(&staging, e))?;
    fs::rename(&staging, &file).map_err(|e| cache_error(&file, e))?;
    Ok(cached.catalog)
}

/// The cache key of the install: it changes when the archives the import
/// reads change.
///
/// The key lists the format version and, for each archive in mount order,
/// its size and modification time; replacing an archive changes at least
/// one of them.
///
/// # Errors
///
/// [`Error::Archive`] if an archive cannot be read.
pub fn cache_key(install: &Path) -> Result<String, Error> {
    let mut key = format!("v{CACHE_FORMAT}");
    for name in ARCHIVES {
        let path = archive_path(install, name);
        let meta =
            fs::metadata(&path).map_err(|e| archive_error(&path, supercom_scd::Error::Io(e)))?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        let _ = write!(key, ";{name}:{}:{modified}", meta.len());
    }
    Ok(key)
}
