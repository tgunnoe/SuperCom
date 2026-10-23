//! The committed skirmish log on `SCMP_012` and the cross-architecture cases
//! (docs/specs/determinism.md DET-024, DET-027 to DET-029). Game-dependent:
//! each test skips, loudly, without the install and fails if
//! `SUPERCOM_REQUIRE_GAME=1`. Counts, identifiers, and digests only.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::path::Path;
use std::process::Command as Process;
use std::sync::Arc;

use supercom_replay_harness::{
    fixture_path, load, nan_case, read_log, recursion_case, run, write_log,
};
use supercom_replay_harness::{hex, report};
use supercom_sim::{Replay, Tick, World, first_divergence};

fn world(fa: &Path) -> (tempfile::TempDir, Arc<World>) {
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load(fa, cache.path()).expect("SCMP_012 loads");
    (cache, world)
}

/// DET-027: two runs of the committed log agree at every tick and with the
/// checkpoints an earlier process wrote.
#[test]
fn conformance_sim_replay_digests_equal() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (_cache, world) = world(&fa);
    let replay = read_log().expect("the committed log reads");
    let (first, _) = run(Arc::clone(&world), &replay).expect("the first run");
    let (second, _) = run(world, &replay).expect("the second run");
    assert_eq!(first_divergence(&first, &second), None);
    assert_eq!(first, second);
    assert!(
        !replay.checkpoints.is_empty(),
        "the log carries checkpoints"
    );
    for (tick, digest) in &replay.checkpoints {
        let index = usize::try_from(tick.0).expect("a tick fits an index");
        assert_eq!(
            first.get(index),
            Some(digest),
            "checkpoint at tick {}",
            tick.0
        );
    }
    let last = replay.checkpoints.last().expect("a last checkpoint").0;
    assert_eq!(
        first.len() as u64,
        last.0 + 1,
        "the run ends at the last checkpoint"
    );
}

/// DET-028: one command moved one tick later changes the digests from the
/// earlier of the two ticks on and not before.
#[test]
fn conformance_sim_replay_diverges_at_altered_tick() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (_cache, world) = world(&fa);
    let replay = read_log().expect("the committed log reads");
    let (baseline, _) = run(Arc::clone(&world), &replay).expect("the baseline run");

    let index = replay
        .commands
        .iter()
        .position(|c| c.tick > Tick(0))
        .expect("a command after tick 0");
    let stamp = replay.commands[index].tick;
    let mut altered: Replay = replay.clone();
    altered.commands[index].tick = Tick(stamp.0 + 1);
    altered.commands.sort_by_key(|c| (c.tick, c.source, c.seq));
    let (moved, _) = run(world, &altered).expect("the altered run");

    assert_eq!(
        first_divergence(&baseline, &moved),
        Some(usize::try_from(stamp.0).expect("a tick"))
    );
    let at = usize::try_from(stamp.0).expect("a tick");
    assert_eq!(baseline[..at], moved[..at], "equal before the earlier tick");
    assert_ne!(baseline[at], moved[at], "different at the earlier tick");
}

/// DET-029: the committed log reaches every counter of the manifest.
#[test]
fn conformance_sim_replay_covers_manifest() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (_cache, world) = world(&fa);
    let replay = read_log().expect("the committed log reads");
    let (_, state) = run(world, &replay).expect("the run");
    let c = &state.counters;
    assert!(
        c.moves_completed > 0,
        "moves completed: {}",
        c.moves_completed
    );
    assert!(
        c.builds_completed > 0,
        "builds completed: {}",
        c.builds_completed
    );
    assert!(
        c.projectile_impacts > 0,
        "impacts: {}",
        c.projectile_impacts
    );
    assert!(c.deaths > 0, "deaths: {}", c.deaths);
    assert!(c.stall_ticks >= 1, "stall ticks: {}", c.stall_ticks);
    assert_eq!(c.defeats, 1, "defeats");
    assert_eq!(c.victories, 1, "victories");
}

/// DET-024: the two cases are repeatable in one process, the NaN case
/// leaves a NaN in the position, and the recursion case names the trap.
#[test]
fn conformance_sim_replay_cases_are_repeatable() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (_cache, world) = world(&fa);
    let first = nan_case(&world).expect("the NaN case");
    let second = nan_case(&world).expect("the NaN case again");
    assert_eq!(first.lines, second.lines);
    assert_eq!(first.lines.len(), 10, "ten stepped ticks");
    assert!(first.position_is_nan, "the position keeps a NaN component");
    assert_eq!(
        first.plugin_nan_bits, 0x7ff8_0000_0000_0000,
        "the host canonicalizes the plugin's NaN"
    );

    let a = recursion_case().expect("the recursion case");
    let b = recursion_case().expect("the recursion case again");
    assert_eq!(a.line, b.line);
    assert!(a.line.contains("call-stack-exhausted"), "{}", a.line);
    assert!(a.depth > 0, "a measured depth");
    eprintln!("depth {}", a.depth);
}

/// DET-024: what the digest program compares is exactly the digest of every
/// tick through the log's last checkpoint, the ten NaN-case lines, and the
/// recursion line, in that order; the NaN bits it prints on stderr are the
/// ones the case placed in the position.
#[test]
fn conformance_sim_replay_report_shape() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let out = report(&fa).expect("the report");
    let replay = read_log().expect("the committed log reads");
    let last = replay.checkpoints.last().expect("a last checkpoint").0;
    let ticks = usize::try_from(last.0).expect("a tick fits an index") + 1;
    assert_eq!(out.stdout.len(), ticks + 10 + 1, "one line per output");
    let (cache, w) = world(&fa);
    let (digests, _) = run(Arc::clone(&w), &replay).expect("the run");
    for (t, line) in out.stdout[..ticks].iter().enumerate() {
        assert_eq!(*line, format!("tick {t} {}", hex(&digests[t])));
        assert_eq!(line.len(), format!("tick {t} ").len() + 64);
    }
    for (i, line) in out.stdout[ticks..ticks + 10].iter().enumerate() {
        assert!(
            line.starts_with(&format!("case nan tick {} ", 50 + i)),
            "{line}"
        );
    }
    assert_eq!(
        out.stdout[ticks + 10],
        "case recursion unbounded trap call-stack-exhausted"
    );

    let nan = nan_case(&w).expect("the NaN case");
    assert_eq!(nan.final_position_bits[0], nan.host_nan_bits);
    assert_eq!(nan.final_position_bits[1], nan.plugin_nan_bits);
    if cfg!(target_arch = "x86_64") {
        assert_ne!(
            nan.host_nan_bits, 0x7ff8_0000_0000_0000,
            "x86_64's own 0/0 is the negative quiet NaN, so the case tests the canonicalizer"
        );
    }
    drop(cache);
}

/// The generator reproduces the committed log byte for byte.
#[test]
fn conformance_sim_replay_generator_reproduces_log() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("regenerated.replay");
    write_log(&fa, &path).expect("the log is written");
    let written = std::fs::read(&path).expect("the written log reads");
    let committed = std::fs::read(fixture_path()).expect("the committed log reads");
    assert!(
        written == committed,
        "the regenerated log differs from the fixture"
    );
}

/// DET-027: the digest program, run as two processes, prints equal bytes.
#[test]
fn conformance_sim_replay_two_processes_agree() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let program = env!("CARGO_BIN_EXE_replay-digests");
    let outputs: Vec<_> = (0..2)
        .map(|_| {
            Process::new(program)
                .env("SUPERCOM_FA_DIR", &fa)
                .output()
                .expect("the digest program starts")
        })
        .collect();
    for out in &outputs {
        assert!(
            out.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert!(
        !outputs[0].stdout.is_empty(),
        "the program prints the digests"
    );
    assert_eq!(outputs[0].stdout, outputs[1].stdout);
}

/// Writes `fixtures/skirmish_scmp_012.replay` from the script, with the
/// checkpoints of its own run. Run once, by hand, after the script or the
/// rules change: `cargo test -p supercom-replay-harness --test replay --
/// --ignored regenerate`.
#[test]
#[ignore = "writes the committed fixture; run by hand"]
fn regenerate_skirmish_log() {
    let fa = supercom_testkit::require_game().expect("the install is required to generate");
    write_log(&fa, &fixture_path()).expect("the log is written");
}
