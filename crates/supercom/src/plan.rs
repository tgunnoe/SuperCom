//! The part of a run that needs no window: from the install to a parsed map.

use crate::error::RunError;
use crate::mapfile::choose_map_file;
use std::fs;
use std::path::Path;
use supercom_scmap::Map;

/// Finds, reads, and parses the map in `folder` of the install at
/// `game_dir`, the install as the binary resolves it.
///
/// # Errors
/// [`RunError`] for an unresolved or missing install, a folder that cannot
/// be listed, a folder without exactly one map file, an unreadable file, or
/// a file that does not parse.
pub fn plan_run(game_dir: Option<&Path>, folder: &str) -> Result<Map, RunError> {
    let dir = game_dir.ok_or(RunError::InstallUnresolved)?;
    if !dir.is_dir() {
        return Err(RunError::InstallMissing {
            dir: dir.to_path_buf(),
        });
    }
    let folder_path = dir.join("maps").join(folder);
    let listing = |error: std::io::Error| RunError::MapFolder {
        folder: folder.to_owned(),
        reason: error.to_string(),
    };
    let names = fs::read_dir(&folder_path)
        .map_err(listing)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(listing)?;
    let path = folder_path.join(choose_map_file(folder, &names)?);
    let bytes = fs::read(&path).map_err(|error| RunError::ReadMap {
        path: path.clone(),
        reason: error.to_string(),
    })?;
    Map::parse(&bytes).map_err(|error| RunError::ParseMap {
        path,
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests;
