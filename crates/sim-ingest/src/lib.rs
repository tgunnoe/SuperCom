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

use supercom_bp::Value;
use supercom_scd::Vfs;
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
    /// The archive layer could not mount the install or read the armour
    /// table.
    Archive(supercom_scd::Error),
    /// The armour table's text did not parse.
    ArmourParse(supercom_bp::Error),
    /// The armour table parsed but is not a table of named multiplier
    /// strings.
    Armour { reason: String },
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
            Self::Archive(e) => write!(f, "cannot read the armour table: {e}"),
            Self::ArmourParse(e) => write!(f, "cannot parse the armour table: {e}"),
            Self::Armour { reason } => write!(f, "malformed armour table: {reason}"),
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
            Self::Archive(e) => Some(e),
            Self::ArmourParse(e) => Some(e),
            Self::Sim(e) => Some(e),
            Self::MissingMapFile { .. } | Self::AmbiguousMapFile { .. } | Self::Armour { .. } => {
                None
            }
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
    // Only the surface is read: the deep and abyss elevations stay on the
    // parsed map (SIM-070).
    let grid = MapGrid::from_heightmap_with_water(
        heightmap.width,
        heightmap.height,
        &heightmap.samples,
        heightmap.scale,
        map.water.map(|w| w.elevation),
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

    let armour = read_armour(install)?;
    World::from_parts_with_armour(
        &catalog.units,
        &catalog.projectiles,
        grid,
        markers,
        map_folder.to_owned(),
        &armour,
    )
    .map(Arc::new)
    .map_err(Error::Sim)
}

/// The armour table of `lua/armordefinition.lua`, read through the archive
/// layer's mount order: each entry is an armour name and its
/// `(damage type, multiplier)` pairs, from strings such as `Normal 1.0`.
fn read_armour(install: &Path) -> Result<Vec<(String, Vec<(String, f64)>)>, Error> {
    let malformed = |reason: String| Error::Armour { reason };
    let bytes = Vfs::from_install(install)
        .and_then(|vfs| vfs.read("lua/armordefinition.lua"))
        .map_err(Error::Archive)?;
    // The installed Lua is ISO-8859 text: one byte, one character.
    let text: String = bytes.iter().map(|&b| char::from(b)).collect();
    let assignments = supercom_bp::parse_script(&text).map_err(Error::ArmourParse)?;
    let Some((_, Value::Table(rows))) = assignments
        .iter()
        .find(|(name, _)| name == "armordefinition")
    else {
        return Err(malformed("no armordefinition table".to_owned()));
    };
    rows.iter()
        .map(|(_, row)| {
            let Value::Table(items) = row else {
                return Err(malformed("an armour entry is not a table".to_owned()));
            };
            let mut strings = items.iter().map(|(_, item)| match item {
                Value::Str(s) => Ok(s.as_str()),
                other => Err(malformed(format!("expected a string, found {other:?}"))),
            });
            let name = strings
                .next()
                .ok_or_else(|| malformed("an armour entry is empty".to_owned()))??;
            let multipliers = strings
                .map(|s| {
                    let s = s?;
                    let mut words = s.split_whitespace();
                    let (Some(damage_type), Some(value), None) =
                        (words.next(), words.next(), words.next())
                    else {
                        return Err(malformed(format!(
                            "{name}: {s:?} is not a damage type and a multiplier"
                        )));
                    };
                    let value = value
                        .parse::<f64>()
                        .map_err(|e| malformed(format!("{name}: {value:?}: {e}")))?;
                    Ok((damage_type.to_owned(), value))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            Ok((name.to_owned(), multipliers))
        })
        .collect()
}

/// The bytes of the one file in `folder` whose name ends `suffix`, ignoring
/// case. The match is by suffix because a folder's files need not carry the
/// folder's name as their prefix.
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

#[cfg(test)]
mod map_file_tests;
