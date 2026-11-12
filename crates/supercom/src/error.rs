//! Why a run of the `view` subcommand ended.

use crate::mapfile::MapFileError;
use std::fmt;
use std::path::PathBuf;

/// A failure of the `view` subcommand, in the order the run can meet them.
#[expect(dead_code, reason = "the window loop constructs the rest")]
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
            Self::Window { reason } => write!(f, "window: {reason}"),
            Self::Render(error) => write!(f, "renderer: {error}"),
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MapFile(error) => Some(error),
            Self::Render(error) => Some(error),
            _ => None,
        }
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
