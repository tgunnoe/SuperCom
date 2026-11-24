use super::*;
use crate::cache::CacheDirError;
use crate::testing::{Scratch, committed_log};
use std::fs;
use supercom_sim::{Army, REPLAY_FORMAT_VERSION, ReplayHeader, Tick, read_replay, write_replay};

fn header(map: &str) -> ReplayHeader {
    ReplayHeader {
        format_version: REPLAY_FORMAT_VERSION,
        engine_version: "test".to_owned(),
        map: map.to_owned(),
        map_digest: [0; 32],
        catalog_digest: [0; 32],
        plugins: Vec::new(),
        players: vec![(Army(1), "UEF".to_owned()), (Army(2), "Cybran".to_owned())],
        seed: 7,
    }
}

fn bytes(map: &str, checkpoints: Vec<(Tick, [u8; 32])>) -> Vec<u8> {
    let replay = Replay {
        header: header(map),
        commands: Vec::new(),
        checkpoints,
    };
    let mut out = Vec::new();
    write_replay(&replay, &mut out);
    out
}

fn write(dir: &Path, name: &str, contents: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    let _ = fs::write(&path, contents);
    path
}

fn error_text(result: Result<Prepared, RunError>) -> Option<(RunError, String)> {
    result.err().map(|error| {
        let text = error.to_string();
        (error, text)
    })
}

/// Each step before the world that needs no installed game gives its own
/// failure, and the text names the step.
#[test]
fn supercom_prepare_replay_failures() {
    let root = Scratch::new("prepare");
    let vars = root.cache_vars();
    let good = write(
        root.path(),
        "good.replay",
        &bytes("SCMP_012", vec![(Tick(99), [1; 32])]),
    );

    assert_eq!(
        error_text(prepare_replay(None, &good, vars.clone())).map(|e| e.0),
        Some(RunError::InstallUnresolved)
    );
    let missing = root.path().join("no-such-install");
    assert_eq!(
        error_text(prepare_replay(Some(&missing), &good, vars.clone())).map(|e| e.0),
        Some(RunError::InstallMissing { dir: missing })
    );

    let absent = root.path().join("absent.replay");
    let read = error_text(prepare_replay(Some(root.path()), &absent, vars.clone()));
    assert!(
        matches!(&read, Some((RunError::ReplayRead { path, .. }, text))
            if *path == absent && text.contains("replay file")),
        "{read:?}"
    );

    let junk = write(root.path(), "junk.replay", b"this is not a replay");
    let parse = error_text(prepare_replay(Some(root.path()), &junk, vars.clone()));
    assert!(
        matches!(&parse, Some((RunError::ReplayParse { path, .. }, text))
            if *path == junk && text.contains("replay file")),
        "{parse:?}"
    );

    let empty = write(root.path(), "empty.replay", &bytes("SCMP_012", Vec::new()));
    let none = error_text(prepare_replay(Some(root.path()), &empty, vars.clone()));
    assert!(
        matches!(&none, Some((RunError::ReplayNoCheckpoints { path }, text))
            if *path == empty && text.contains("checkpoint")),
        "{none:?}"
    );

    let slash = write(
        root.path(),
        "slash.replay",
        &bytes("a/b", vec![(Tick(99), [1; 32])]),
    );
    let folder = error_text(prepare_replay(Some(root.path()), &slash, vars.clone()));
    assert!(
        matches!(&folder, Some((RunError::ReplayMapFolder { folder }, text))
            if folder == "a/b" && text.contains("replay map folder")),
        "{folder:?}"
    );

    let unresolved = error_text(prepare_replay(
        Some(root.path()),
        &good,
        CacheVars::default(),
    ));
    assert!(
        matches!(&unresolved, Some((RunError::CacheDir(CacheDirError::Unresolved), text))
            if text.contains("cache directory")),
        "{unresolved:?}"
    );
    let relative = CacheVars {
        override_dir: Some("relative-cache".into()),
        ..CacheVars::default()
    };
    let relative = error_text(prepare_replay(Some(root.path()), &good, relative));
    assert!(
        matches!(
            &relative,
            Some((RunError::CacheDir(CacheDirError::Relative { .. }), _))
        ),
        "{relative:?}"
    );

    let world = error_text(prepare_replay(Some(root.path()), &good, vars));
    assert!(
        matches!(&world, Some((RunError::World { .. }, text)) if text.starts_with("world:")),
        "an install holding nothing gives no world: {world:?}"
    );

    // The step after the map file: an install whose archives do not mount.
    let mounted = crate::units::resolve_kinds(root.path(), &["uel0001".to_owned()]);
    assert!(
        matches!(&mounted, Err(RunError::UnitArchives { .. })),
        "an install holding nothing mounts no archives: {mounted:?}"
    );
}

/// Two defects at once give the earlier step's failure, in the order of
/// REN-057.
#[test]
fn supercom_prepare_replay_reports_the_earlier_of_two_failures() {
    let root = Scratch::new("prepare-order");
    let slash = write(
        root.path(),
        "slash.replay",
        &bytes("a/b", vec![(Tick(99), [1; 32])]),
    );
    let no_install = root.path().join("no-such-install");
    let missing_and_unreadable = error_text(prepare_replay(
        Some(&no_install),
        &root.path().join("absent.replay"),
        CacheVars::default(),
    ));
    assert_eq!(
        missing_and_unreadable.map(|e| e.0),
        Some(RunError::InstallMissing { dir: no_install }),
        "the install is checked before the replay file is read"
    );
    let no_checkpoint_and_slash =
        write(root.path(), "empty-slash.replay", &bytes("a/b", Vec::new()));
    let first = error_text(prepare_replay(
        Some(root.path()),
        &no_checkpoint_and_slash,
        CacheVars::default(),
    ));
    assert!(
        matches!(&first, Some((RunError::ReplayNoCheckpoints { path }, _))
            if *path == no_checkpoint_and_slash),
        "a replay with no checkpoint is refused before its map folder is checked: {first:?}"
    );
    let slash_and_no_cache = error_text(prepare_replay(
        Some(root.path()),
        &slash,
        CacheVars::default(),
    ));
    assert!(
        matches!(&slash_and_no_cache, Some((RunError::ReplayMapFolder { folder }, _))
            if folder == "a/b"),
        "the map folder is checked before the cache directory: {slash_and_no_cache:?}"
    );
}

/// Against the install: the committed log prepares, a changed header does
/// not, and the content check names itself.
#[test]
fn conformance_prepare_replay_checks_content_then_starts() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let root = Scratch::new("prepare-conformance");
    let log = committed_log();
    let prepared = prepare_replay(Some(&fa), &log, root.cache_vars());
    assert!(prepared.is_ok(), "{:?}", prepared.as_ref().err());
    let Ok(prepared) = prepared else { return };
    assert_eq!(prepared.state.tick, Tick(0));
    assert_eq!(prepared.state.entities.len(), 2);
    assert_eq!(prepared.replay.checkpoints.len(), 46);
    assert_eq!(
        (prepared.map.heightmap.width, prepared.map.heightmap.height),
        (256, 256)
    );

    let original = fs::read(&log).unwrap_or_default();
    let parsed = read_replay(&original);
    assert!(
        parsed.is_ok(),
        "the committed log reads: {:?}",
        parsed.as_ref().err()
    );
    let Ok(mut replay) = parsed else { return };
    replay.header.map_digest[0] ^= 1;
    let mut changed = Vec::new();
    write_replay(&replay, &mut changed);
    let altered = write(root.path(), "altered.replay", &changed);
    let refused = error_text(prepare_replay(Some(&fa), &altered, root.cache_vars()));
    assert!(
        matches!(&refused, Some((RunError::ContentCheck { .. }, text))
            if text.starts_with("content check and start state:")),
        "{refused:?}"
    );
}

/// The committed log names five kinds, resolved after the map file: the
/// model set is the markers and the five in sorted order; a log that names a
/// kind the install lacks ends the run at that step, after every step before
/// it, with an error naming the kind.
#[test]
fn conformance_prepare_replay_resolves_the_named_kinds() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let root = Scratch::new("prepare-kinds");
    let log = committed_log();
    let prepared = prepare_replay(Some(&fa), &log, root.cache_vars());
    assert!(prepared.is_ok(), "{:?}", prepared.as_ref().err());
    let Ok(prepared) = prepared else { return };
    let five = ["ueb1103", "ueb2101", "uel0001", "urb1101", "url0001"];
    assert_eq!(
        crate::units::run_kinds(&prepared.state, &prepared.replay),
        five
    );
    assert_eq!(prepared.meshes.model_set().len(), 9);
    for (place, kind) in five.iter().enumerate() {
        assert_eq!(
            prepared.meshes.get(kind).map(|(index, _)| index),
            Some(4 + place),
            "{kind}"
        );
    }

    let parsed = read_replay(&fs::read(&log).unwrap_or_default());
    assert!(
        parsed.is_ok(),
        "the committed log reads: {:?}",
        parsed.as_ref().err()
    );
    let Ok(mut replay) = parsed else { return };
    let last = replay
        .commands
        .last()
        .map(|c| (c.tick, c.source, c.seq, c.army));
    let (tick, source, seq, army) = last.unwrap_or((Tick(0), 0, 0, Army(1)));
    replay.commands.push(supercom_sim::Command {
        tick,
        source,
        seq: seq + 1,
        army,
        units: Vec::new(),
        order: supercom_sim::Order::BuildFactory {
            blueprint: "NoSuchKind".to_owned(),
        },
        mode: supercom_sim::QueueMode::Replace,
    });
    let mut changed = Vec::new();
    write_replay(&replay, &mut changed);
    let altered = write(root.path(), "unknown-kind.replay", &changed);
    let refused = error_text(prepare_replay(Some(&fa), &altered, root.cache_vars()));
    assert!(
        matches!(&refused, Some((RunError::UnitKind { kind, step: crate::error::KindStep::MeshRead, .. }, text))
            if kind == "nosuchkind" && text.starts_with("unit kind nosuchkind: mesh read")),
        "{refused:?}"
    );
}
