//! Helpers the binary's unit tests share: scratch directories under the
//! system's temporary directory, and the committed log's path.

use crate::cache::CacheVars;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A fresh directory under the system's temporary directory, removed on
/// drop.
pub struct Scratch(PathBuf);

impl Scratch {
    /// Makes the directory, named for `label`, the process, and a counter.
    pub fn new(label: &str) -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("supercom-{label}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the scratch directory is created");
        Self(dir)
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// The cache variables that name a cache directory inside this one.
    pub fn cache_vars(&self) -> CacheVars {
        CacheVars {
            override_dir: Some(self.0.join("cache").into_os_string()),
            ..CacheVars::default()
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The committed log, by a path built from the manifest's directory.
pub fn committed_log() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../replay-harness/fixtures/skirmish_scmp_012.replay")
}
