//! The part of a run that needs no window: from the install to a parsed map.

use crate::error::RunError;
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
    let _ = (game_dir, folder);
    Err(RunError::InstallUnresolved)
}

#[cfg(test)]
mod tests;
