//! The replay harness (DET-027 to DET-029, DET-024): the committed skirmish
//! log, its reader and runner, and the two cross-architecture cases. Evidence
//! tooling, never shipped.
//!
//! The log is a skirmish between a UEF and a Cybran commander on `SCMP_012`,
//! scripted below and written once by [`write_log`]; its player strings name
//! the factions (DET-030), so the file and the installed data start it.

mod cases;
mod script;

use core::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use supercom_sim::{Digest, Replay, State, Tick, World, read_replay, run_replay, write_replay};

pub use cases::{NanCase, RecursionCase, nan_case, recursion_case};

/// The map folder the log is played on.
pub const MAP: &str = "SCMP_012";

/// Why a harness step did not complete.
#[derive(Debug)]
pub enum HarnessError {
    /// A file could not be read or written.
    Io { path: PathBuf, reason: String },
    /// The installed data could not be loaded.
    Load(String),
    /// The sim refused a replay, a command, or a tick.
    Sim(supercom_sim::Error),
    /// The plugin host refused a component or a call.
    Plugin(supercom_plugin::Error),
    /// The script cannot be built on this world, or a case did not behave.
    Case(String),
    /// The game install was not found.
    NoInstall,
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, reason } => write!(f, "{}: {reason}", path.display()),
            Self::Load(reason) => write!(f, "loading {MAP}: {reason}"),
            Self::Sim(err) => write!(f, "the sim: {err}"),
            Self::Plugin(err) => write!(f, "the plugin host: {err}"),
            Self::Case(reason) => f.write_str(reason),
            Self::NoInstall => f.write_str(
                "no Forged Alliance install found; set SUPERCOM_FA_DIR to its directory",
            ),
        }
    }
}

impl core::error::Error for HarnessError {}

impl From<supercom_sim::Error> for HarnessError {
    fn from(err: supercom_sim::Error) -> Self {
        Self::Sim(err)
    }
}

impl From<supercom_plugin::Error> for HarnessError {
    fn from(err: supercom_plugin::Error) -> Self {
        Self::Plugin(err)
    }
}

/// The committed log's path.
#[must_use]
pub fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/skirmish_scmp_012.replay")
}

/// Read the committed log at run time, so the crate compiles before the
/// file exists.
///
/// # Errors
///
/// [`HarnessError`] when the file is absent or is not a replay.
pub fn read_log() -> Result<Replay, HarnessError> {
    let path = fixture_path();
    let bytes = fs::read(&path).map_err(|err| HarnessError::Io {
        path: path.clone(),
        reason: err.to_string(),
    })?;
    Ok(read_replay(&bytes)?)
}

/// Load `SCMP_012` from the install at `install`, caching the catalog under
/// `cache`.
///
/// # Errors
///
/// [`HarnessError::Load`] when the install cannot be loaded.
pub fn load(install: &Path, cache: &Path) -> Result<Arc<World>, HarnessError> {
    supercom_sim_ingest::load_world(install, cache, MAP)
        .map_err(|err| HarnessError::Load(err.to_string()))
}

/// Run `replay` through its last checkpoint tick, checking the world against
/// its header first (DET-007).
///
/// # Errors
///
/// [`HarnessError`] from the sim, or a replay with no checkpoint to say
/// where it ends.
pub fn run(world: Arc<World>, replay: &Replay) -> Result<(Vec<Digest>, State), HarnessError> {
    let (last, _) = replay
        .checkpoints
        .last()
        .ok_or_else(|| HarnessError::Case("the log has no checkpoint to end it".to_owned()))?;
    Ok(run_replay(world, replay, last.0.saturating_add(1))?)
}

/// Run the scripted skirmish on the install at `install` and write its log,
/// checkpoints from its own run, to `path`. The generator's one call.
///
/// # Errors
///
/// [`HarnessError`] when the world, the script, the run, or the write fails.
pub fn write_log(install: &Path, path: &Path) -> Result<(), HarnessError> {
    let cache = tempfile::tempdir().map_err(|err| HarnessError::Io {
        path: PathBuf::from("<temporary cache>"),
        reason: err.to_string(),
    })?;
    let world = load(install, cache.path())?;
    let mut replay = script::replay(&world)?;
    let (digests, _) = run_replay(Arc::clone(&world), &replay, script::LOG_TICKS)?;
    replay.checkpoints = (0..)
        .zip(&digests)
        .filter(|(tick, _)| script::is_checkpoint(*tick))
        .map(|(tick, digest)| (Tick(tick), *digest))
        .collect();
    let mut bytes = Vec::new();
    write_replay(&replay, &mut bytes);
    fs::write(path, bytes).map_err(|err| HarnessError::Io {
        path: path.to_path_buf(),
        reason: err.to_string(),
    })
}

/// One hex string of a digest, 64 lower-case digits.
#[must_use]
pub fn hex(digest: &Digest) -> String {
    digest.iter().fold(String::with_capacity(64), |mut out, b| {
        // Writing to a String cannot fail.
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// What the digest program prints: `stdout` is host-independent, `stderr`
/// informative (DET-024).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
}

/// The digest program's whole output on the install at `install`.
///
/// # Errors
///
/// [`HarnessError`] from any step.
pub fn report(install: &Path) -> Result<Report, HarnessError> {
    let cache = tempfile::tempdir().map_err(|err| HarnessError::Io {
        path: PathBuf::from("<temporary cache>"),
        reason: err.to_string(),
    })?;
    let world = load(install, cache.path())?;
    let replay = read_log()?;
    let (digests, _) = run(Arc::clone(&world), &replay)?;
    let mut out = Report::default();
    for (tick, digest) in digests.iter().enumerate() {
        out.stdout.push(format!("tick {tick} {}", hex(digest)));
    }
    let nan = nan_case(&world)?;
    out.stdout.extend(nan.lines);
    out.stderr
        .push(format!("nan host bits {:#018x}", nan.host_nan_bits));
    out.stderr
        .push(format!("nan plugin bits {:#018x}", nan.plugin_nan_bits));
    out.stderr.push(format!(
        "nan final position bits {:#018x} {:#018x} {:#018x}",
        nan.final_position_bits[0], nan.final_position_bits[1], nan.final_position_bits[2]
    ));
    let recursion = recursion_case()?;
    out.stdout.push(recursion.line);
    out.stderr.push(format!("depth {}", recursion.depth));
    Ok(out)
}
