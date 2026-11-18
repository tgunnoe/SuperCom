//! Why a run of the `view` or `replay` subcommand ended.

use crate::cache::CacheDirError;
use crate::mapfile::MapFileError;
use std::fmt;
use std::path::PathBuf;

/// A failure of the `view` or `replay` subcommand, in the order the run can
/// meet them.
#[derive(Clone, Debug, PartialEq)]
pub enum RunError {
    /// The window system could not be reached.
    NoDisplay {
        /// What winit reported.
        reason: String,
    },
    /// Neither `SUPERCOM_FA_DIR` nor a home directory names an install.
    InstallUnresolved,
    /// The install directory does not exist.
    InstallMissing {
        /// The directory the binary resolved.
        dir: PathBuf,
    },
    /// The map folder could not be listed.
    MapFolder {
        /// The folder as given.
        folder: String,
        /// What the file system reported.
        reason: String,
    },
    /// The folder holds no map file, or more than one.
    MapFile(MapFileError),
    /// The map file could not be read.
    ReadMap {
        /// The file.
        path: PathBuf,
        /// What the file system reported.
        reason: String,
    },
    /// The map file does not parse.
    ParseMap {
        /// The file.
        path: PathBuf,
        /// What the parser reported.
        reason: String,
    },
    /// The replay file could not be read.
    ReplayRead {
        /// The file.
        path: PathBuf,
        /// What the file system reported.
        reason: String,
    },
    /// The replay file does not parse.
    ReplayParse {
        /// The file.
        path: PathBuf,
        /// What the parser reported.
        reason: String,
    },
    /// The replay holds no checkpoint, so the run would have no end.
    ReplayNoCheckpoints {
        /// The file.
        path: PathBuf,
    },
    /// The replay's header names a map that is not a single folder name.
    ReplayMapFolder {
        /// The name as the header has it.
        folder: String,
    },
    /// No cache directory could be chosen.
    CacheDir(CacheDirError),
    /// The world could not be loaded.
    World {
        /// What the loader reported.
        reason: String,
    },
    /// The replay does not fit the world, or its start state cannot be made.
    ContentCheck {
        /// What the simulation reported.
        reason: String,
    },
    /// A step's digest differs from the replay's checkpoint.
    Mismatch {
        /// The tick of the checkpoint.
        tick: u64,
    },
    /// The simulation refused a step.
    Step {
        /// The tick being run.
        tick: u64,
        /// What the simulation reported.
        reason: String,
    },
    /// The window could not be created or has failed.
    Window {
        /// What went wrong.
        reason: String,
    },
    /// The renderer refused or failed.
    Render(supercom_render::Error),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDisplay { reason } => write!(
                f,
                "no display is available ({reason}); set WAYLAND_DISPLAY, WAYLAND_SOCKET, or DISPLAY"
            ),
            Self::InstallUnresolved => write!(
                f,
                "game data: unresolved; set SUPERCOM_FA_DIR to your install"
            ),
            Self::InstallMissing { dir } => write!(
                f,
                "game data: {} (missing; set SUPERCOM_FA_DIR to your install)",
                dir.display()
            ),
            Self::MapFolder { folder, reason } => {
                write!(f, "map folder {folder}: {reason}")
            }
            Self::MapFile(error) => write!(f, "{error}"),
            Self::ReadMap { path, reason } => {
                write!(f, "cannot read {}: {reason}", path.display())
            }
            Self::ParseMap { path, reason } => {
                write!(f, "cannot parse {}: {reason}", path.display())
            }
            Self::ReplayRead { path, reason } => {
                write!(f, "replay file {}: cannot read: {reason}", path.display())
            }
            Self::ReplayParse { path, reason } => {
                write!(f, "replay file {}: cannot parse: {reason}", path.display())
            }
            Self::ReplayNoCheckpoints { path } => write!(
                f,
                "replay file {}: holds no checkpoint, so the run would have no end",
                path.display()
            ),
            Self::ReplayMapFolder { folder } => {
                write!(f, "replay map folder {folder:?}: not a single folder name")
            }
            Self::CacheDir(error) => write!(f, "{error}"),
            Self::World { reason } => write!(f, "world: cannot load: {reason}"),
            Self::ContentCheck { reason } => {
                write!(f, "content check and start state: {reason}")
            }
            Self::Mismatch { tick } => write!(
                f,
                "replay: the digest at tick {tick} differs from its checkpoint"
            ),
            Self::Step { tick, reason } => {
                write!(f, "replay: the step at tick {tick} failed: {reason}")
            }
            Self::Window { reason } => write!(f, "window: {reason}"),
            Self::Render(error) => write!(f, "renderer: {error}"),
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MapFile(error) => Some(error),
            Self::CacheDir(error) => Some(error),
            Self::Render(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CacheDirError> for RunError {
    fn from(error: CacheDirError) -> Self {
        Self::CacheDir(error)
    }
}

impl From<MapFileError> for RunError {
    fn from(error: MapFileError) -> Self {
        Self::MapFile(error)
    }
}

impl From<supercom_render::Error> for RunError {
    fn from(error: supercom_render::Error) -> Self {
        Self::Render(error)
    }
}
