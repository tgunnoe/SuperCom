//! The built binary: the command line's failures, the missing display, and
//! one real window when asked.

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

/// With no argument the binary prints its version and reports the install.
#[test]
fn supercom_bare_run_reports_the_install() {
    let output = Command::new(BINARY)
        .env("SUPERCOM_FA_DIR", NO_INSTALL)
        .output();
    let Ok(output) = output else {
        fail("the binary runs");
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.starts_with(&format!("supercom {}", env!("CARGO_PKG_VERSION"))),
        "the version line comes first: {stdout:?}"
    );
    assert!(stderr.contains(NO_INSTALL), "{stderr:?}");
    assert_eq!(output.status.code(), Some(1));
}

/// A usage error is reported before the display and the install are touched.
#[test]
fn supercom_usage_error_precedes_display_and_install() {
    for args in [
        &["view", "--frames", "0"][..],
        &["view", "../etc"],
        &["bogus"],
    ] {
        let output = Command::new(BINARY)
            .args(args)
            .env("SUPERCOM_FA_DIR", NO_INSTALL)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .output();
        let Ok(output) = output else {
            fail("the binary runs");
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(stderr.contains("usage:"), "{args:?}: {stderr}");
        assert!(!stderr.contains("display"), "{args:?}: {stderr}");
        assert!(!stderr.contains(NO_INSTALL), "{args:?}: {stderr}");
    }
}

/// Where no display is found except through these variables, `view` with
/// none of them set fails with a message and no panic, before the install
/// is read. It runs everywhere on Linux, the gate included, and opens no
/// window.
#[cfg(target_os = "linux")]
#[test]
fn supercom_view_without_display_fails_cleanly() {
    let output = Command::new(BINARY)
        .arg("view")
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
        "the install is not read before the display is checked: {stderr}"
    );
}

/// A guard that passes without its flag: with `SUPERCOM_REQUIRE_DISPLAY=1`
/// it opens a real window on the display it inherits, so it is run only
/// inside a nested compositor, and presents three frames of an installed
/// map. Without the flag it prints `SKIP: <reason>` and passes.
#[test]
fn conformance_window_presents_frames() {
    if std::env::var("SUPERCOM_REQUIRE_DISPLAY").as_deref() != Ok("1") {
        println!("SKIP: set SUPERCOM_REQUIRE_DISPLAY=1 to open a window");
        return;
    }
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let spawned = Command::new(BINARY)
        .args(["view", "SCMP_012", "--frames", "3"])
        .env("SUPERCOM_FA_DIR", fa)
        .stdout(Stdio::piped())
        .spawn();
    let Ok(mut child) = spawned else {
        fail("the binary starts");
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                fail("the window did not finish within a minute");
            }
            Err(error) => fail(&format!("waiting for the binary failed: {error}")),
        }
    };
    let output = child.wait_with_output();
    let stdout = output
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    assert!(status.success(), "exit {status}; stdout {stdout:?}");
    assert!(
        stdout.lines().any(|line| line == "presented 3 frames"),
        "{stdout:?}"
    );
}
