//! Choosing the map file of a map folder.

use std::ffi::OsString;
use std::fmt;

/// The folder does not hold exactly one map file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapFileError {
    /// No file name ends in `.scmap`.
    NoMapFile {
        /// The folder as given.
        folder: String,
    },
    /// More than one file name ends in `.scmap`.
    AmbiguousMapFile {
        /// The folder as given.
        folder: String,
        /// The matching names, sorted.
        names: Vec<String>,
    },
}

impl fmt::Display for MapFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMapFile { folder } => write!(f, "map folder {folder} holds no .scmap file"),
            Self::AmbiguousMapFile { folder, names } => write!(
                f,
                "map folder {folder} holds more than one .scmap file: {}",
                names.join(", ")
            ),
        }
    }
}

impl std::error::Error for MapFileError {}

/// The one name among `names` that ends in `.scmap`, ignoring case.
///
/// # Errors
/// [`MapFileError`] when no name matches or more than one does.
pub fn choose_map_file(folder: &str, names: &[OsString]) -> Result<OsString, MapFileError> {
    let mut matches: Vec<&OsString> = names
        .iter()
        .filter(|name| {
            name.to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".scmap")
        })
        .collect();
    matches.sort();
    match matches.as_slice() {
        [] => Err(MapFileError::NoMapFile {
            folder: folder.to_owned(),
        }),
        [only] => Ok((*only).clone()),
        _ => Err(MapFileError::AmbiguousMapFile {
            folder: folder.to_owned(),
            names: matches
                .iter()
                .map(|name| name.to_string_lossy().into_owned())
                .collect(),
        }),
    }
}

#[cfg(test)]
mod tests;
