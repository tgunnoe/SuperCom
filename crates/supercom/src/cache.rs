//! The directory the world's importer caches into: a pure function of three
//! environment values.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

/// The environment values the rule reads; an empty value counts as unset.
#[derive(Clone, Debug, Default)]
pub struct CacheVars {
    /// `SUPERCOM_CACHE_DIR`: the directory itself.
    pub override_dir: Option<OsString>,
    /// `XDG_CACHE_HOME`: the directory `supercom` is joined to.
    pub xdg_cache_home: Option<OsString>,
    /// `HOME`: the directory `.cache/supercom` is joined to.
    pub home: Option<OsString>,
}

/// Why no cache directory could be chosen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheDirError {
    /// None of the three values is set and non-empty.
    Unresolved,
    /// The directory the rule gave is not an absolute path.
    Relative {
        /// The directory the rule gave.
        path: PathBuf,
    },
}

impl fmt::Display for CacheDirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unresolved => write!(
                f,
                "cache directory: none of SUPERCOM_CACHE_DIR, XDG_CACHE_HOME, or HOME is set"
            ),
            Self::Relative { path } => write!(
                f,
                "cache directory: {} is not an absolute path",
                path.display()
            ),
        }
    }
}

impl std::error::Error for CacheDirError {}

/// The three cache variables as `lookup` answers them, each read under its
/// own name: `SUPERCOM_CACHE_DIR`, `XDG_CACHE_HOME`, and `HOME`. A name
/// `lookup` does not answer leaves its field unset.
#[must_use]
pub fn cache_vars_from(lookup: &dyn Fn(&str) -> Option<OsString>) -> CacheVars {
    CacheVars {
        override_dir: lookup("SUPERCOM_CACHE_DIR"),
        xdg_cache_home: lookup("XDG_CACHE_HOME"),
        home: lookup("HOME"),
    }
}

/// The cache directory: `SUPERCOM_CACHE_DIR` itself, else `XDG_CACHE_HOME`
/// joined with `supercom`, else `HOME` joined with `.cache/supercom`.
///
/// # Errors
/// [`CacheDirError::Unresolved`] when no value is set and non-empty, and
/// [`CacheDirError::Relative`] when the directory the rule gives is not
/// absolute; nothing falls back to the working directory.
pub fn cache_dir(vars: &CacheVars) -> Result<PathBuf, CacheDirError> {
    let set = |value: &Option<OsString>| value.clone().filter(|value| !value.is_empty());
    let chosen = if let Some(dir) = set(&vars.override_dir) {
        PathBuf::from(dir)
    } else if let Some(xdg) = set(&vars.xdg_cache_home) {
        PathBuf::from(xdg).join("supercom")
    } else if let Some(home) = set(&vars.home) {
        PathBuf::from(home).join(".cache/supercom")
    } else {
        return Err(CacheDirError::Unresolved);
    };
    if chosen.is_absolute() {
        Ok(chosen)
    } else {
        Err(CacheDirError::Relative { path: chosen })
    }
}

#[cfg(test)]
mod tests;
