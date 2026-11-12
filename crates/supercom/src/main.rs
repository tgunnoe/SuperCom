//! Entry point of the `supercom` binary.
//!
//! With no argument the binary resolves the game data directory and reports
//! whether it exists. `supercom view` opens a window on an installed map and
//! draws its terrain and water. The engine proper lands behind the
//! specification in `docs/specs/`.

mod cli;
mod error;
#[cfg_attr(not(test), expect(dead_code, reason = "the window loop calls it"))]
mod frame;
#[cfg_attr(not(test), expect(dead_code, reason = "the window loop calls it"))]
mod input;
#[cfg_attr(not(test), expect(dead_code, reason = "the plan calls it"))]
mod mapfile;
#[cfg_attr(not(test), expect(dead_code, reason = "the view subcommand calls it"))]
mod plan;

use cli::Command;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

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
        Ok(Command::View(_)) => ExitCode::SUCCESS,
    }
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
