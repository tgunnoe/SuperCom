//! The steps of a replay run before the window: one function from the
//! install, the replay file, and the cache variables to what the window
//! needs, or an error naming the step that failed.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "wired by the code that follows the tests")
)]

use crate::cache::CacheVars;
use crate::error::RunError;
use std::path::Path;
use supercom_scmap::Map;
use supercom_sim::{Replay, State};

/// What the window needs to play a replay.
pub struct Prepared {
    /// The parsed map, drawn as `view` draws it.
    pub map: Map,
    /// The parsed replay, with its checkpoints.
    pub replay: Replay,
    /// The state at tick 0 of the skirmish the replay's header names.
    pub state: State,
}

/// Runs the steps before the window, in the order of REN-057.
///
/// # Errors
/// [`RunError`] for the first step that fails: an unresolved or missing
/// install, an unreadable or unparsable replay file, a replay with no
/// checkpoint, a header map folder that is not a single component, no usable
/// cache directory, a world that does not load, a replay that does not fit
/// the world, or a map file that does not read or parse.
pub fn prepare_replay(
    game_dir: Option<&Path>,
    file: &Path,
    cache: CacheVars,
) -> Result<Prepared, RunError> {
    let _ = (game_dir, file, cache);
    Err(RunError::InstallUnresolved)
}

#[cfg(test)]
mod tests;
