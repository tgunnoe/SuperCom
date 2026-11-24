//! The built binary's `replay` subcommand: the missing display, and one
//! real window when asked.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[expect(
    clippy::panic,
    reason = "test support; clippy.toml exempts only #[test] functions"
)]
fn fail(message: &str) -> ! {
    panic!("{message}")
}

const BINARY: &str = env!("CARGO_BIN_EXE_supercom");

/// A path no install lives at, so a test that must not read the install
/// shows it did by naming this path.
const NO_INSTALL: &str = "/nonexistent/supercom-no-install";

/// A replay path that does not exist, named in the same way.
const NO_REPLAY: &str = "/nonexistent/supercom-no-replay.replay";

fn committed_log() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../replay-harness/fixtures/skirmish_scmp_012.replay")
}

/// Where no display is found except through the three variables, `replay`
/// with none of them set fails with the message of `view`, before the
/// install or the file is read, and a usage error is reported first.
#[cfg(target_os = "linux")]
#[test]
fn supercom_replay_without_display_fails_cleanly() {
    let output = Command::new(BINARY)
        .args(["replay", NO_REPLAY])
        .env("SUPERCOM_FA_DIR", NO_INSTALL)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .output();
    let Ok(output) = output else {
        fail("the binary runs");
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a failure, not a panic (101): {stderr}"
    );
    assert!(stderr.contains("no display is available"), "{stderr}");
    for variable in ["WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY"] {
        assert!(stderr.contains(variable), "{variable} is named: {stderr}");
    }
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(
        !stderr.contains(NO_INSTALL),
        "the install is not read: {stderr}"
    );
    assert!(
        !stderr.contains(NO_REPLAY),
        "the file is not read: {stderr}"
    );

    let usage = Command::new(BINARY)
        .args(["replay", NO_REPLAY, "--speed", "10.01"])
        .env("SUPERCOM_FA_DIR", NO_INSTALL)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .output();
    let Ok(usage) = usage else {
        fail("the binary runs");
    };
    let stderr = String::from_utf8_lossy(&usage.stderr);
    assert_eq!(usage.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("usage:"), "{stderr}");
    assert!(!stderr.contains("no display"), "{stderr}");
}

/// A guard that passes without its flag: with `SUPERCOM_REQUIRE_DISPLAY=1`
/// it opens a real window on the display it inherits, so it is run only
/// inside a nested compositor, and presents three frames of the committed
/// log. Without the flag it prints `SKIP: <reason>` and passes.
#[test]
fn conformance_window_replay_prints_summary_then_frames() {
    if std::env::var("SUPERCOM_REQUIRE_DISPLAY").as_deref() != Ok("1") {
        println!("SKIP: set SUPERCOM_REQUIRE_DISPLAY=1 to open a window");
        return;
    }
    let Some(fa) = supercom_testkit::require_game() else {
        fail(
            "SUPERCOM_REQUIRE_DISPLAY=1 is set but no game install was found; \
             set SUPERCOM_FA_DIR to the install",
        );
    };
    let cache = std::env::temp_dir().join(format!("supercom-replay-window-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    let spawned = Command::new(BINARY)
        .arg("replay")
        .arg(committed_log())
        .args(["--frames", "3"])
        .env("SUPERCOM_FA_DIR", fa)
        .env("SUPERCOM_CACHE_DIR", &cache)
        .stdout(Stdio::piped())
        .spawn();
    let Ok(mut child) = spawned else {
        fail("the binary starts");
    };
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(&cache);
                fail("the window did not finish within two minutes");
            }
            Err(error) => fail(&format!("waiting for the binary failed: {error}")),
        }
    };
    let output = child.wait_with_output();
    let _ = std::fs::remove_dir_all(&cache);
    let stdout = output
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    assert!(status.success(), "exit {status}; stdout {stdout:?}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.last().copied(),
        Some("presented 3 frames"),
        "{stdout:?}"
    );
    let summaries: Vec<&&str> = lines
        .iter()
        .filter(|line| line.starts_with("replayed "))
        .collect();
    assert_eq!(summaries.len(), 1, "one summary line: {stdout:?}");
    assert!(
        summaries[0].contains(" of 46 checkpoints matched, "),
        "{stdout:?}"
    );
    assert_eq!(
        lines.iter().position(|line| line.starts_with("replayed ")),
        lines.len().checked_sub(2),
        "the summary comes just before the frame count: {stdout:?}"
    );
}

/// The whole committed log at ten times the speed in one real window, run
/// only under `SUPERCOM_REQUIRE_DISPLAY=1` inside a nested compositor that
/// shows nothing: 4,000 frames pass the last checkpoint (the match takes
/// about 46 seconds at this speed), and the summary line is written once,
/// with 46 of 46 checkpoints matched and no kind drawn as a marker because
/// it was not named.
#[test]
fn conformance_window_replay_at_ten_times_prints_one_summary() {
    if std::env::var("SUPERCOM_REQUIRE_DISPLAY").as_deref() != Ok("1") {
        println!("SKIP: set SUPERCOM_REQUIRE_DISPLAY=1 to open a window");
        return;
    }
    let Some(fa) = supercom_testkit::require_game() else {
        fail(
            "SUPERCOM_REQUIRE_DISPLAY=1 is set but no game install was found; \
             set SUPERCOM_FA_DIR to the install",
        );
    };
    let cache = std::env::temp_dir().join(format!("supercom-replay-ten-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    let spawned = Command::new(BINARY)
        .arg("replay")
        .arg(committed_log())
        .args(["--speed", "10", "--frames", "4000"])
        .env("SUPERCOM_FA_DIR", fa)
        .env("SUPERCOM_CACHE_DIR", &cache)
        .stdout(Stdio::piped())
        .spawn();
    let Ok(mut child) = spawned else {
        fail("the binary starts");
    };
    let deadline = Instant::now() + Duration::from_secs(300);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(&cache);
                fail("the window did not finish within five minutes");
            }
            Err(error) => fail(&format!("waiting for the binary failed: {error}")),
        }
    };
    let output = child.wait_with_output();
    let _ = std::fs::remove_dir_all(&cache);
    let stdout = output
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    assert!(status.success(), "exit {status}; stdout {stdout:?}");
    let lines: Vec<&str> = stdout.lines().collect();
    let summaries: Vec<&&str> = lines
        .iter()
        .filter(|line| line.starts_with("replayed "))
        .collect();
    assert_eq!(summaries.len(), 1, "exactly one summary line: {stdout:?}");
    assert!(
        summaries[0].starts_with("replayed 4600 ticks, 46 of 46 checkpoints matched, "),
        "{stdout:?}"
    );
    assert!(
        summaries[0].ends_with(" frames held back, 0 kinds drawn as markers"),
        "{stdout:?}"
    );
    assert_eq!(
        lines.last().copied(),
        Some("presented 4000 frames"),
        "{stdout:?}"
    );
}

/// A replay file ends with its checkpoints: a count as a `u32`, then for each
/// its tick as a little-endian `u64` and its 32-byte digest (DET-026). The
/// committed log holds 46, the first at tick 99.
const CHECKPOINTS: usize = 46;
const CHECKPOINT_BYTES: usize = 8 + 32;

/// The committed log with one bit of its first checkpoint's digest changed.
fn log_with_wrong_first_digest() -> Vec<u8> {
    let Ok(mut bytes) = std::fs::read(committed_log()) else {
        fail("the committed log reads");
    };
    let Some(first) = bytes.len().checked_sub(CHECKPOINTS * CHECKPOINT_BYTES) else {
        fail("the committed log is shorter than its checkpoints");
    };
    let tick = bytes
        .get(first..first + 8)
        .and_then(|b| <[u8; 8]>::try_from(b).ok());
    assert_eq!(tick.map(u64::from_le_bytes), Some(99), "the first tick");
    let count = first
        .checked_sub(4)
        .and_then(|from| bytes.get(from..first))
        .and_then(|b| <[u8; 4]>::try_from(b).ok());
    assert_eq!(
        count.map(u32::from_le_bytes),
        Some(46),
        "the checkpoint count precedes the first checkpoint"
    );
    let Some(digest_byte) = bytes.get_mut(first + 8) else {
        fail("the first digest is in the file");
    };
    *digest_byte ^= 1;
    bytes
}

/// A run that ends in a mismatch writes no summary line (REN-063): the log
/// with its first checkpoint's digest changed, at ten times the speed in one
/// real window, run only under `SUPERCOM_REQUIRE_DISPLAY=1` inside a nested
/// compositor that shows nothing, ends with the failure code and a message
/// on stderr naming tick 99, and nothing on stdout begins `replayed `.
#[test]
fn conformance_window_replay_mismatch_writes_no_summary() {
    if std::env::var("SUPERCOM_REQUIRE_DISPLAY").as_deref() != Ok("1") {
        println!("SKIP: set SUPERCOM_REQUIRE_DISPLAY=1 to open a window");
        return;
    }
    let Some(fa) = supercom_testkit::require_game() else {
        fail(
            "SUPERCOM_REQUIRE_DISPLAY=1 is set but no game install was found; \
             set SUPERCOM_FA_DIR to the install",
        );
    };
    let dir = std::env::temp_dir().join(format!("supercom-replay-mismatch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    if std::fs::create_dir_all(&dir).is_err() {
        fail("the temporary directory is made");
    }
    let altered = dir.join("wrong-digest.replay");
    if std::fs::write(&altered, log_with_wrong_first_digest()).is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        fail("the altered copy is written");
    }
    let spawned = Command::new(BINARY)
        .arg("replay")
        .arg(&altered)
        .args(["--speed", "10"])
        .env("SUPERCOM_FA_DIR", fa)
        .env("SUPERCOM_CACHE_DIR", dir.join("cache"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let Ok(mut child) = spawned else {
        let _ = std::fs::remove_dir_all(&dir);
        fail("the binary starts");
    };
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(&dir);
                fail("the window did not finish within two minutes");
            }
            Err(error) => {
                let _ = std::fs::remove_dir_all(&dir);
                fail(&format!("waiting for the binary failed: {error}"));
            }
        }
    };
    let output = child.wait_with_output();
    let _ = std::fs::remove_dir_all(&dir);
    let Ok(output) = output else {
        fail("the binary's output is read");
    };
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(!status.success(), "a mismatch fails the run: {stdout:?}");
    assert_eq!(status.code(), Some(1), "stderr {stderr:?}");
    assert!(stderr.contains("tick 99"), "stderr {stderr:?}");
    assert!(
        !stdout.lines().any(|line| line.starts_with("replayed ")),
        "no summary line after a mismatch: {stdout:?}"
    );
}
