//! Sandbox guarantees of `supercom-plugin`, pinned against the `hello`
//! fixture in `plugins/hello`.
//!
//! The fixture is a complete plugin component. The first test in a process to
//! need it builds it with cargo for `wasm32-unknown-unknown` and encodes it
//! with `wasm-tools`, both found on `PATH`; the build lands under the
//! directory cargo reserves for integration tests. Every test then loads its
//! own fresh [`Plugin`] from the cached bytes, so no test depends on an
//! instance another test has used.
//!
//! A failure to build or encode the fixture is a fault in this harness, not in
//! the host: the helper panics with the tool's captured output, and the suite
//! is red for the right reason only while that never happens.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers, and panic! is not exempt anywhere"
)]

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use supercom_plugin::{Error, Host, Plugin};

/// Manifest of the fixture, a standalone cargo project outside the workspace.
const MANIFEST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../plugins/hello/Cargo.toml"
);

/// Where the fixture is built (`--target-dir`): under the directory cargo
/// reserves for integration tests.
const BUILD_DIR: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/hello");

/// Serialises the processes that build into `BUILD_DIR`.
const BUILD_LOCK: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/hello.lock");

/// The canonical quiet `NaN` of an `f64`: sign clear, only the top bit of the
/// payload set.
const CANONICAL_NAN: u64 = 0x7ff8_0000_0000_0000;

/// The fixture in both forms.
struct Fixture {
    /// The core module exactly as cargo emits it: valid WebAssembly, but not
    /// a component.
    core: Vec<u8>,
    /// The core module encoded as a component.
    component: Vec<u8>,
}

/// Runs `command`, panicking with its captured output unless it succeeds.
fn run(command: &mut Command) {
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("cannot start {command:?}: {err}"));
    assert!(
        output.status.success(),
        "{command:?} failed with {}\n--- stdout\n{}\n--- stderr\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Reads `path`, panicking with the path when it cannot be read.
fn read(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
}

/// Builds the fixture and encodes it as a component.
///
/// nextest gives every test its own process, so several can arrive here at
/// once. An exclusive lock on a file beside the build directory lets one at a
/// time build, encode, and read, so that none reads a file another is writing;
/// the later ones find cargo's build fresh.
fn build_fixture() -> Fixture {
    let dir = Path::new(BUILD_DIR);
    fs::create_dir_all(dir).expect("create the fixture build directory");
    let build_lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(BUILD_LOCK)
        .expect("open the fixture build lock");
    build_lock.lock().expect("take the fixture build lock");

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    run(Command::new(cargo)
        .args([
            "build",
            "--release",
            "--locked",
            "--manifest-path",
            MANIFEST,
        ])
        .args(["--target", "wasm32-unknown-unknown", "--target-dir"])
        .arg(dir));

    let core_path = dir.join("wasm32-unknown-unknown/release/hello.wasm");
    let component_path = dir.join("hello.component.wasm");
    run(Command::new("wasm-tools")
        .args(["component", "new"])
        .arg(&core_path)
        .arg("-o")
        .arg(&component_path));

    // The lock is released when `build_lock` drops, after both files are read.
    Fixture {
        core: read(&core_path),
        component: read(&component_path),
    }
}

/// The fixture, built once per test process.
fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(build_fixture)
}

/// A fresh host and a fresh instance of the component loaded from the cached
/// bytes. The fixture is built first, so a broken harness shows before any
/// failure of the host does.
fn fresh() -> Plugin {
    let component = &fixture().component;
    let host = Host::new().expect("Host::new");
    host.load(component).expect("load the hello component")
}

/// The component imports exactly the engine's own `host` interface and
/// nothing else: no `wasi:` interface, and with it no clock, randomness, or
/// file system, can reach a plugin (AGENTS.md I3).
#[test]
fn sandbox_component_imports_only_the_engine_host() {
    let component = &fixture().component;
    let host = Host::new().expect("Host::new");
    let imports = host
        .imports(component)
        .expect("list the imports of the hello component");
    assert_eq!(imports, ["supercom:engine/host@0.1.0"]);
}

/// A loaded plugin answers `describe` with its own text, so the linker
/// satisfies the `host.log` import the fixture calls on the way.
#[test]
fn sandbox_describe_returns_the_plugins_text() {
    let mut plugin = fresh();
    assert_eq!(plugin.describe().expect("describe"), "hello");
}

/// A call that stays within its fuel returns the plugin's result unchanged:
/// `step` echoes its tick.
#[test]
fn sandbox_step_within_fuel_returns_the_result() {
    let mut plugin = fresh();
    assert_eq!(plugin.step(7, 1_000_000).expect("step"), 7);
}

/// `NaN` canonicalization is on: a `NaN` the guest computes as `0.0 / 0.0`
/// reaches the host as the canonical quiet `NaN`, not as whatever pattern the
/// hardware produced, so two platforms replaying one command stream cannot
/// diverge on it (AGENTS.md R2).
#[test]
fn sandbox_nan_bits_are_canonical() {
    let mut plugin = fresh();
    let bits = plugin.nan_bits(1_000_000).expect("nan_bits");
    assert_eq!(
        bits, CANONICAL_NAN,
        "got {bits:#018x}, want {CANONICAL_NAN:#018x}"
    );
}

/// A guest that never returns is stopped by fuel metering: `spin` with a
/// finite budget fails with `OutOfFuel`, not with a hang and not with a
/// generic `Trap` (AGENTS.md I3).
#[test]
fn sandbox_spin_runs_out_of_fuel() {
    let mut plugin = fresh();
    let result = plugin.spin(10_000);
    assert!(
        matches!(result, Err(Error::OutOfFuel)),
        "spin(10_000) returned {result:?}"
    );
}

/// Fuel exhaustion is deterministic: two fresh loads of the same component,
/// given the same fuel, both stop with `OutOfFuel`. What a second call on the
/// same instance does is unspecified and not tested here.
#[test]
fn sandbox_fuel_exhaustion_is_deterministic() {
    let first = fresh().spin(10_000);
    let second = fresh().spin(10_000);
    assert!(
        matches!(first, Err(Error::OutOfFuel)),
        "first load returned {first:?}"
    );
    assert!(
        matches!(second, Err(Error::OutOfFuel)),
        "second load returned {second:?}"
    );
}

/// A core module is valid WebAssembly but not a component. Loading the
/// fixture's un-encoded core module fails with `NotAComponent`, which tells a
/// mod author who skipped the encoding step what went wrong, rather than
/// reporting a generic `Compile` failure.
#[test]
fn sandbox_core_module_is_not_a_component() {
    let core = &fixture().core;
    let host = Host::new().expect("Host::new");
    let Err(err) = host.load(core) else {
        panic!("loading a core module must fail");
    };
    assert!(
        matches!(err, Error::NotAComponent),
        "loading a core module returned {err:?}"
    );
}
