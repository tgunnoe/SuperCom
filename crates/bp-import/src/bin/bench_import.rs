//! Load benchmark for the blueprint import (the campaign's R3 budget).
//!
//! Usage: `SUPERCOM_FA_DIR=<install> bench_import`. Prints one
//! `name seconds` line each for the cold import, the cache write, the cached
//! load, and `mount_share`, the time to mount the whole install through the
//! archive layer's `Vfs::from_install`, measured on its own: the import never
//! uses the Vfs and opens the six archives it needs directly, so that line
//! bounds the import's own mount cost rather than measuring it.
//! Exits 1 when the cold import takes over 10 s or the cached load over 1 s,
//! and 2 when it cannot run. Blueprints only: map Lua is never timed.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// Budget for a cold import of the v0 set.
const COLD_BUDGET: Duration = Duration::from_secs(10);

/// Budget for a load from the cache.
const CACHED_BUDGET: Duration = Duration::from_secs(1);

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(reason) => {
            eprintln!("bench_import: {reason}");
            ExitCode::from(2)
        }
    }
}

/// Runs the four measurements; `true` when both budgets hold.
fn run() -> Result<bool, String> {
    let install = std::env::var_os("SUPERCOM_FA_DIR")
        .map(PathBuf::from)
        .ok_or("set SUPERCOM_FA_DIR to the install directory")?;
    let cache_dir = std::env::temp_dir().join(format!("supercom-bench-{}", std::process::id()));

    let mount_started = Instant::now();
    let mounted = supercom_scd::Vfs::from_install(&install).map_err(|e| e.to_string());
    let mount = mount_started.elapsed();
    mounted?;

    let cold_started = Instant::now();
    supercom_bp_import::import(&install).map_err(|e| e.to_string())?;
    let cold = cold_started.elapsed();

    let write_started = Instant::now();
    supercom_bp_import::load(&install, &cache_dir).map_err(|e| e.to_string())?;
    let write = write_started.elapsed();

    let cached_started = Instant::now();
    let cached_result = supercom_bp_import::load(&install, &cache_dir).map_err(|e| e.to_string());
    let cached = cached_started.elapsed();
    // Best effort: the directory is scratch space under the temp dir.
    let _ = std::fs::remove_dir_all(&cache_dir);
    cached_result?;

    println!("cold_import {:.3}", cold.as_secs_f64());
    println!("cache_write {:.3}", write.as_secs_f64());
    println!("cached_load {:.3}", cached.as_secs_f64());
    println!("mount_share {:.3}", mount.as_secs_f64());
    Ok(cold <= COLD_BUDGET && cached <= CACHED_BUDGET)
}
