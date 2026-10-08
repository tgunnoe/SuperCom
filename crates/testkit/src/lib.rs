//! Test support for the `SuperCom` workspace.
//!
//! Tests that read the installed Forged Alliance data are game-dependent:
//! they run against the real install when it is present and skip, loudly,
//! when it is not. This crate decides which, in two layers. Pure helpers
//! ([`resolve_game_dir`], [`decide_skip`]) take every environment input as an
//! argument, so their logic is unit-testable without touching the process
//! environment. Thin wrappers ([`game_dir`], [`require_game`]) feed them the
//! real environment.
//!
//! Game-dependent tests are named `conformance_*` and begin with:
//!
//! ```text
//! let Some(fa) = supercom_testkit::require_game() else { return };
//! ```
//!
//! SKELETON: every function here is a deliberately unimplemented stub. The
//! unit tests in this crate and the `conformance_*` suites of the crates that
//! depend on it fail against the stubs until they are implemented.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// What a game-dependent test should do, as decided by [`decide_skip`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gate {
    /// The game is installed at this directory: run the test against it.
    Run(PathBuf),
    /// The game is absent and not required: print `SKIP: <reason>` to stderr
    /// and return.
    Skip(String),
    /// The game is absent but required: fail the test with this reason.
    Fail(String),
}

/// Resolves the game install directory from explicit inputs.
///
/// A non-empty `override_dir` (the value of `SUPERCOM_FA_DIR`) is the only
/// candidate when it is set, even if it does not exist: a stale override never
/// falls back to the Steam default, which is how a run simulates a machine
/// without the game. An empty override counts as unset. Otherwise the
/// candidate is the Steam default beneath `home`. The candidate is returned
/// only when `exists` accepts it.
#[must_use]
pub fn resolve_game_dir(
    _override_dir: Option<OsString>,
    _home: Option<OsString>,
    _exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    None
}

/// Decides what a game-dependent test does, given the resolved directory and
/// whether the game is required (`SUPERCOM_REQUIRE_GAME=1`).
///
/// A present game always runs. An absent game skips unless required, in which
/// case it fails; both carry the same reason, which names `SUPERCOM_FA_DIR` so
/// the reader knows how to point the suite at an install.
#[must_use]
pub fn decide_skip(_dir: Option<PathBuf>, _require: bool) -> Gate {
    Gate::Run(PathBuf::new())
}

/// The installed game's directory: `SUPERCOM_FA_DIR` if set, else the Steam
/// default beneath `HOME`; `None` when that directory does not exist.
#[must_use]
pub fn game_dir() -> Option<PathBuf> {
    None
}

/// The installed game's directory for a game-dependent test, or `None` to
/// skip it.
///
/// When the game is absent this prints `SKIP: <reason>` to stderr and returns
/// `None`, so the test passes visibly skipped (nextest has no runtime skip).
///
/// # Panics
///
/// When `SUPERCOM_REQUIRE_GAME=1` and the game is absent, with the reason, so
/// a gate that must exercise the game cannot pass vacuously.
///
/// STUB: returns an empty directory, never `None`, so that game-dependent
/// tests run and fail on the unimplemented capability under test rather than
/// returning early and passing.
#[must_use]
pub fn require_game() -> Option<PathBuf> {
    Some(PathBuf::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn everything_exists(_: &Path) -> bool {
        true
    }

    fn nothing_exists(_: &Path) -> bool {
        false
    }

    fn skip_reason(gate: &Gate) -> Option<&str> {
        match gate {
            Gate::Skip(reason) => Some(reason),
            _ => None,
        }
    }

    fn fail_reason(gate: &Gate) -> Option<&str> {
        match gate {
            Gate::Fail(reason) => Some(reason),
            _ => None,
        }
    }

    /// Pins `resolve_game_dir`: a non-empty override that exists wins over
    /// the home default.
    #[test]
    fn override_wins_over_the_home_default() {
        let dir = resolve_game_dir(
            Some("/games/fa".into()),
            Some("/home/me".into()),
            everything_exists,
        );
        assert_eq!(dir, Some(PathBuf::from("/games/fa")));
    }

    /// Pins `resolve_game_dir`: an empty override is treated exactly as an
    /// unset one, falling through to the home default.
    #[test]
    fn empty_override_is_treated_as_unset() {
        let unset = resolve_game_dir(None, Some("/home/me".into()), everything_exists);
        let empty = resolve_game_dir(
            Some(OsString::new()),
            Some("/home/me".into()),
            everything_exists,
        );
        assert!(unset.is_some(), "an existing default must resolve");
        assert_eq!(empty, unset);
    }

    /// Pins `resolve_game_dir`: the default is a fixed, non-empty relative
    /// path beneath `HOME`, so it moves with `HOME` and nothing else.
    #[test]
    fn default_lies_beneath_home_and_follows_it() {
        let suffix = |home: &str| {
            let dir = resolve_game_dir(None, Some(home.into()), everything_exists)?;
            dir.strip_prefix(home).ok().map(Path::to_path_buf)
        };
        let first = suffix("/home/me");
        let second = suffix("/srv/another-user");
        assert!(
            first
                .as_ref()
                .is_some_and(|relative| relative.components().count() > 0),
            "the default must be a non-empty path beneath HOME, got {first:?}"
        );
        assert_eq!(first, second, "the default suffix must not depend on HOME");
    }

    /// Pins `resolve_game_dir`: a set override that does not exist yields
    /// `None` and never falls back to the home default, even when that
    /// default exists. The control arm shows the same override resolving once
    /// it exists.
    #[test]
    fn missing_override_does_not_fall_back_to_the_default() {
        let all_but_nowhere = |dir: &Path| dir != Path::new("/nowhere");
        let stale = resolve_game_dir(
            Some("/nowhere".into()),
            Some("/home/me".into()),
            all_but_nowhere,
        );
        let control = resolve_game_dir(
            Some("/somewhere".into()),
            Some("/home/me".into()),
            all_but_nowhere,
        );
        assert_eq!(control, Some(PathBuf::from("/somewhere")), "control arm");
        assert_eq!(stale, None, "a stale override must not fall back");
    }

    /// Pins `resolve_game_dir`: a default that does not exist yields `None`.
    /// The control arm shows the same inputs resolving once it exists.
    #[test]
    fn missing_default_resolves_to_none() {
        let present = resolve_game_dir(None, Some("/home/me".into()), everything_exists);
        let absent = resolve_game_dir(None, Some("/home/me".into()), nothing_exists);
        assert!(
            present.is_some(),
            "control arm: an existing default resolves"
        );
        assert_eq!(absent, None);
    }

    /// Pins `resolve_game_dir`: with neither an override nor `HOME` there is
    /// nothing to resolve. The control arm shows `HOME` alone sufficing.
    #[test]
    fn no_override_and_no_home_resolves_to_none() {
        let with_home = resolve_game_dir(None, Some("/home/me".into()), everything_exists);
        let without = resolve_game_dir(None, None, everything_exists);
        assert!(with_home.is_some(), "control arm: HOME alone resolves");
        assert_eq!(without, None);
    }

    /// Pins `resolve_game_dir`: `exists` is asked about the candidate
    /// directory and nothing else, once, for the override and for the
    /// default alike.
    #[test]
    fn existence_is_probed_on_the_candidate_only() {
        let probed = RefCell::new(Vec::new());
        let record = |dir: &Path| {
            probed.borrow_mut().push(dir.to_path_buf());
            true
        };

        let from_override =
            resolve_game_dir(Some("/games/fa".into()), Some("/home/me".into()), record);
        assert_eq!(from_override, Some(PathBuf::from("/games/fa")));
        assert_eq!(*probed.borrow(), [PathBuf::from("/games/fa")]);

        probed.borrow_mut().clear();
        let from_default = resolve_game_dir(None, Some("/home/me".into()), record);
        let expected: Vec<PathBuf> = from_default.into_iter().collect();
        assert_eq!(expected.len(), 1, "the default must resolve");
        assert_eq!(*probed.borrow(), expected);
    }

    /// Pins `decide_skip`: a present game runs against that directory
    /// whether or not the game is required.
    #[test]
    fn present_game_runs_whether_or_not_it_is_required() {
        for require in [false, true] {
            assert_eq!(
                decide_skip(Some(PathBuf::from("/games/fa")), require),
                Gate::Run(PathBuf::from("/games/fa")),
                "require = {require}"
            );
        }
    }

    /// Pins `decide_skip`: an absent, optional game skips, with a reason
    /// that names `SUPERCOM_FA_DIR`.
    #[test]
    fn absent_game_skips_and_names_the_override_variable() {
        let gate = decide_skip(None, false);
        assert!(
            skip_reason(&gate).is_some_and(|reason| reason.contains("SUPERCOM_FA_DIR")),
            "expected a skip naming SUPERCOM_FA_DIR, got {gate:?}"
        );
    }

    /// Pins `decide_skip`: an absent but required game fails, with the same
    /// reason a skip would carry.
    #[test]
    fn absent_required_game_fails_with_the_skip_reason() {
        let required = decide_skip(None, true);
        let optional = decide_skip(None, false);
        assert!(
            fail_reason(&required).is_some_and(|reason| reason.contains("SUPERCOM_FA_DIR")),
            "expected a failure naming SUPERCOM_FA_DIR, got {required:?}"
        );
        assert_eq!(fail_reason(&required), skip_reason(&optional));
    }
}
