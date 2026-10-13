//! Builds the simulation's static world from the installed game.
//!
//! `supercom-sim` forbids the filesystem, so every file the world is built
//! from is read here: the blueprint catalog through `supercom-bp-import`, the
//! map's `.scmap` and `_save.lua` through `supercom-scmap` and `supercom-bp`.
//! The parsed values are converted into the sim's types and handed to
//! `World::from_parts`, which computes both digests (SIM-049 to SIM-051).

use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use supercom_sim::{MapGrid, Marker, MarkerKind, Pos, World};

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
    /// The map folder holds several files of the given suffix.
    AmbiguousMapFile {
        folder: String,
        suffix: String,
        count: usize,
    },
    /// The `.scmap` did not parse.
    Map(supercom_scmap::Error),
    /// The save file did not parse.
    Save(supercom_bp::Error),
    /// The sim refused the parsed values.
    Sim(supercom_sim::Error),
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
            Self::AmbiguousMapFile {
                folder,
                suffix,
                count,
            } => write!(f, "map folder {folder} has {count} files ending {suffix}"),
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
            Self::MissingMapFile { .. } | Self::AmbiguousMapFile { .. } => None,
        }
    }
}

/// Build the world for `map_folder` (a folder name under `<install>/maps`)
/// from the installed catalog, caching the catalog under `cache_dir`. The
/// folder's `.scmap` and `_save.lua` are matched case-insensitively by
/// suffix, and each must match exactly one file.
///
/// # Errors
///
/// Any [`Error`] variant: a failed import, an unreadable, missing, or
/// ambiguous map file, a parse failure, or a value the sim refuses.
pub fn load_world(install: &Path, cache_dir: &Path, map_folder: &str) -> Result<Arc<World>, Error> {
    let catalog = supercom_bp_import::load(install, cache_dir).map_err(Error::Catalog)?;
    let folder = install.join("maps").join(map_folder);

    let scmap = read_map_file(&folder, map_folder, ".scmap")?;
    let map = supercom_scmap::Map::parse(&scmap).map_err(Error::Map)?;
    let heightmap = &map.heightmap;
    let grid = MapGrid::from_heightmap(
        heightmap.width,
        heightmap.height,
        &heightmap.samples,
        heightmap.scale,
    )
    .map_err(Error::Sim)?;

    let save = read_map_file(&folder, map_folder, "_save.lua")?;
    let text = String::from_utf8_lossy(&save);
    let assignments = supercom_bp::parse_script(&text).map_err(Error::Save)?;
    let markers = supercom_bp::markers(&assignments)
        .map_err(Error::Save)?
        .iter()
        .map(convert_marker)
        .collect();

    World::from_parts(
        &catalog.units,
        &catalog.projectiles,
        grid,
        markers,
        map_folder.to_owned(),
    )
    .map(Arc::new)
    .map_err(Error::Sim)
}

/// The bytes of the one file in `folder` whose name ends `suffix`, ignoring
/// case; 23 of the installed map folders use lower-case prefixes.
fn read_map_file(folder: &Path, name: &str, suffix: &str) -> Result<Vec<u8>, Error> {
    let io = |path: &Path, source| Error::Io {
        path: path.display().to_string(),
        source,
    };
    let mut found = Vec::new();
    for entry in fs::read_dir(folder).map_err(|e| io(folder, e))? {
        let path = entry.map_err(|e| io(folder, e))?.path();
        let matches = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.to_lowercase().ends_with(suffix));
        if matches {
            found.push(path);
        }
    }
    match found.as_slice() {
        [path] => fs::read(path).map_err(|e| io(path, e)),
        [] => Err(Error::MissingMapFile {
            folder: name.to_owned(),
            suffix: suffix.to_owned(),
        }),
        several => Err(Error::AmbiguousMapFile {
            folder: name.to_owned(),
            suffix: suffix.to_owned(),
            count: several.len(),
        }),
    }
}

/// A save-file marker as the sim holds it. A name `ARMY_n` with `n` a
/// decimal from 1 to 255 marks army slot `n` whatever its type string (the
/// installed army markers carry the type `Blank Marker`); otherwise the type
/// `Mass` or `Hydrocarbon` names the resource and anything else is `Other`.
fn convert_marker(marker: &supercom_bp::Marker) -> Marker {
    let kind = match army_slot(&marker.name) {
        Some(slot) => MarkerKind::Army(slot),
        None => match marker.kind.as_str() {
            "Mass" => MarkerKind::Mass,
            "Hydrocarbon" => MarkerKind::Hydrocarbon,
            _ => MarkerKind::Other,
        },
    };
    let [x, y, z] = marker.position;
    Marker {
        name: marker.name.clone(),
        kind,
        pos: Pos {
            x: f64::from(x),
            y: f64::from(y),
            z: f64::from(z),
        },
    }
}

fn army_slot(name: &str) -> Option<u8> {
    let digits = name.strip_prefix("ARMY_")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u8>().ok().filter(|n| *n >= 1)
}
