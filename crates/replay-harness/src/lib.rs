//! The replay harness (DET-027 to DET-029, DET-024): the committed skirmish
//! log, its reader and runner, and the two cross-architecture cases. Evidence
//! tooling, never shipped. Every function here is a stub at the red tip.

use core::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use supercom_sim::{Digest, Replay, State, World};

/// Why a harness step did not complete.
#[derive(Debug)]
pub enum HarnessError {
    /// The step is not written yet.
    Unimplemented(&'static str),
    /// A file could not be read or written.
    Io { path: PathBuf, reason: String },
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unimplemented(what) => write!(f, "{what} is not implemented"),
            Self::Io { path, reason } => write!(f, "{}: {reason}", path.display()),
        }
    }
}

impl core::error::Error for HarnessError {}

/// What the payload-NaN case reports (DET-024).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NanCase {
    /// The compared lines: one `case nan tick <t> <hex>` per stepped tick.
    pub lines: Vec<String>,
    /// The bits of the NaN the host's arithmetic produced, for stderr.
    pub host_nan_bits: u64,
    /// The bits of the NaN the plugin returned, for stderr.
    pub plugin_nan_bits: u64,
    /// The bits of the extra entity's final position, for stderr.
    pub final_position_bits: [u64; 3],
    /// Whether a component of that position is still a NaN.
    pub position_is_nan: bool,
}

/// What the recursion case reports (DET-024).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecursionCase {
    /// The compared line: the kind of trap an unbounded recursion raises.
    pub line: String,
    /// The largest depth that returned on this host, for stderr.
    pub depth: u64,
}

/// The committed log's path.
#[must_use]
pub fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/skirmish_scmp_012.replay")
}

/// Read the committed log.
///
/// # Errors
///
/// [`HarnessError`] when the file is absent or is not a replay.
pub fn read_log() -> Result<Replay, HarnessError> {
    Err(HarnessError::Unimplemented("read_log"))
}

/// Load `SCMP_012` from the install at `install`, caching under `cache`.
///
/// # Errors
///
/// [`HarnessError`] when the install cannot be loaded.
pub fn load(install: &Path, cache: &Path) -> Result<Arc<World>, HarnessError> {
    let _ = (install, cache);
    Err(HarnessError::Unimplemented("load"))
}

/// Run `replay` through its last checkpoint tick.
///
/// # Errors
///
/// [`HarnessError`] from the sim.
pub fn run(world: Arc<World>, replay: &Replay) -> Result<(Vec<Digest>, State), HarnessError> {
    let _ = (world, replay);
    Err(HarnessError::Unimplemented("run"))
}

/// Run the scripted skirmish on the install at `install` and write its log
/// to `path` (the generator's one call).
///
/// # Errors
///
/// [`HarnessError`] when the script, the run, or the write fails.
pub fn write_log(install: &Path, path: &Path) -> Result<(), HarnessError> {
    let _ = (install, path);
    Err(HarnessError::Unimplemented("write_log"))
}

/// The payload-NaN case on `world`.
///
/// # Errors
///
/// [`HarnessError`] from the sim or the plugin host.
pub fn nan_case(world: &Arc<World>) -> Result<NanCase, HarnessError> {
    let _ = world;
    Err(HarnessError::Unimplemented("nan_case"))
}

/// The recursion case.
///
/// # Errors
///
/// [`HarnessError`] when the host does not trap with stack exhaustion.
pub fn recursion_case() -> Result<RecursionCase, HarnessError> {
    Err(HarnessError::Unimplemented("recursion_case"))
}
