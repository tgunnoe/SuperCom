//! Entry point of the `supercom` binary.
//!
//! With no argument the binary resolves the game data directory and reports
//! whether it exists. `supercom view` opens a window on an installed map and
//! draws its terrain and water. `supercom replay <file>` plays a recorded
//! match in a window: it loads the world, checks the replay against it,
//! steps the simulation, compares every checkpoint, and draws each unit as its
//! lit mesh when the camera is near and as a flat marker otherwise, and every
//! projectile as a flat marker. The engine proper lands behind the
//! specification in `docs/specs/`.

mod cache;
mod cli;
mod error;
mod frame;
mod input;
mod interp;
mod mapfile;
mod markers;
mod pacing;
mod plan;
mod prepare;
mod scene;
mod stepping;
#[cfg(test)]
mod testing;
mod units;
mod window;

use cache::cache_vars_from;
use cli::{Command, ReplayArgs, ViewArgs};
use error::RunError;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use window::Outcome;
use winit::event_loop::EventLoop;

/// Environment variable that overrides the Forged Alliance install directory.
const GAME_DIR_VAR: &str = "SUPERCOM_FA_DIR";

/// Steam's default install location, relative to the home directory.
const STEAM_DEFAULT: &str = ".local/share/Steam/steamapps/common/Supreme Commander Forged Alliance";

/// Resolves the game directory: a non-empty override wins, otherwise the Steam
/// default under `home`. Returns `None` when neither is available.
fn resolve_game_dir(override_dir: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    override_dir
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(STEAM_DEFAULT)))
}

fn main() -> ExitCode {
    match cli::parse_args(env::args_os().skip(1)) {
        Err(usage) => {
            eprintln!("{usage}");
            ExitCode::FAILURE
        }
        Ok(Command::Bare) => bare(),
        Ok(Command::View(args)) => view(&args),
        Ok(Command::Replay(args)) => replay(&args),
    }
}

/// `replay`: the display first, then the steps before the window, then the
/// window.
fn replay(args: &ReplayArgs) -> ExitCode {
    match run_replay(args) {
        Ok(Outcome::Presented(frames)) => {
            println!("presented {frames} frames");
            ExitCode::SUCCESS
        }
        Ok(Outcome::Closed) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("supercom: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_replay(args: &ReplayArgs) -> Result<Outcome, RunError> {
    let event_loop = EventLoop::new().map_err(|error| RunError::NoDisplay {
        reason: error.to_string(),
    })?;
    let game_dir = resolve_game_dir(env::var_os(GAME_DIR_VAR), env::var_os("HOME"));
    let cache = cache_vars_from(&|name| env::var_os(name));
    let prepared = prepare::prepare_replay(game_dir.as_deref(), &args.file, cache)?;
    window::run_replay(event_loop, prepared, args.speed, args.frames)
}

/// `view`: the display first, then the install and the map, then the window.
fn view(args: &ViewArgs) -> ExitCode {
    match run_view(args) {
        Ok(Outcome::Presented(frames)) => {
            println!("presented {frames} frames");
            ExitCode::SUCCESS
        }
        Ok(Outcome::Closed) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("supercom: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_view(args: &ViewArgs) -> Result<Outcome, RunError> {
    let event_loop = EventLoop::new().map_err(|error| RunError::NoDisplay {
        reason: error.to_string(),
    })?;
    let game_dir = resolve_game_dir(env::var_os(GAME_DIR_VAR), env::var_os("HOME"));
    let map = plan::plan_run(game_dir.as_deref(), &args.folder)?;
    window::run(event_loop, map, args.frames)
}

/// The run with no argument: report whether the game data is found.
fn bare() -> ExitCode {
    println!("supercom {}", env!("CARGO_PKG_VERSION"));
    match resolve_game_dir(env::var_os(GAME_DIR_VAR), env::var_os("HOME")) {
        Some(dir) if dir.is_dir() => {
            println!("game data: {} (found)", dir.display());
            ExitCode::SUCCESS
        }
        Some(dir) => {
            eprintln!(
                "game data: {} (missing; set {GAME_DIR_VAR} to your install)",
                dir.display()
            );
            ExitCode::FAILURE
        }
        None => {
            eprintln!("game data: unresolved; set {GAME_DIR_VAR} to your install");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins_over_home() {
        let dir = resolve_game_dir(Some("/games/fa".into()), Some("/home/me".into()));
        assert_eq!(dir, Some(PathBuf::from("/games/fa")));
    }

    #[test]
    fn empty_override_falls_back_to_steam_default() {
        let dir = resolve_game_dir(Some(OsString::new()), Some("/home/me".into()));
        assert_eq!(dir, Some(PathBuf::from("/home/me").join(STEAM_DEFAULT)));
    }

    #[test]
    fn nothing_available_resolves_to_none() {
        assert_eq!(resolve_game_dir(None, None), None);
    }
}
