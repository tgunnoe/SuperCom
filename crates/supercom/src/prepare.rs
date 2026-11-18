//! The steps of a replay run before the window: one function from the
//! install, the replay file, and the cache variables to what the window
//! needs, or an error naming the step that failed.

use crate::cache::{CacheVars, cache_dir};
use crate::cli::is_single_component;
use crate::error::RunError;
use crate::plan::{check_install, read_map};
use crate::units::{Meshes, kinds_named, resolve_kinds};
use std::fs;
use std::path::Path;
use supercom_scmap::Map;
use supercom_sim::{Replay, State, read_replay};
use supercom_sim_ingest::load_world;

/// What the window needs to play a replay.
pub struct Prepared {
    /// The parsed map, drawn as `view` draws it.
    pub map: Map,
    /// The parsed replay, with its checkpoints.
    pub replay: Replay,
    /// The state at tick 0 of the skirmish the replay's header names.
    pub state: State,
    /// The unit kinds the replay names, resolved to meshes and scales.
    pub meshes: Meshes,
}

/// Runs the steps before the window, in the order of REN-057.
///
/// # Errors
/// [`RunError`] for the first step that fails: an unresolved or missing
/// install, an unreadable or unparsable replay file, a replay with no
/// checkpoint, a header map folder that is not a single component, no usable
/// cache directory, a world that does not load, a replay that does not fit
/// the world, a map file that does not read or parse, or a named unit kind
/// that does not resolve to a mesh and a scale.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the caller hands over the environment's values, which the run does not need again"
)]
pub fn prepare_replay(
    game_dir: Option<&Path>,
    file: &Path,
    cache: CacheVars,
) -> Result<Prepared, RunError> {
    let install = check_install(game_dir)?;
    let bytes = fs::read(file).map_err(|error| RunError::ReplayRead {
        path: file.to_path_buf(),
        reason: error.to_string(),
    })?;
    let replay = read_replay(&bytes).map_err(|error| RunError::ReplayParse {
        path: file.to_path_buf(),
        reason: error.to_string(),
    })?;
    if replay.checkpoints.is_empty() {
        return Err(RunError::ReplayNoCheckpoints {
            path: file.to_path_buf(),
        });
    }
    let folder = replay.header.map.as_str();
    if !is_single_component(folder) {
        return Err(RunError::ReplayMapFolder {
            folder: folder.to_owned(),
        });
    }
    let cache = cache_dir(&cache)?;
    let world = load_world(install, &cache, folder).map_err(|error| RunError::World {
        reason: error.to_string(),
    })?;
    let state = world
        .replay_start(&replay.header)
        .map_err(|error| RunError::ContentCheck {
            reason: error.to_string(),
        })?;
    let map = read_map(install, folder)?;
    let meshes = resolve_kinds(install, &kinds_named(&replay))?;
    Ok(Prepared {
        map,
        replay,
        state,
        meshes,
    })
}

#[cfg(test)]
mod tests;
