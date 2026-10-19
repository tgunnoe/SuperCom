//! The replay-start constructor and the pure runner (docs/specs/determinism.md
//! DET-027, DET-030). All synthetic: no install, no file read.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::sync::Arc;

use supercom_sim::{
    Army, Command, Digest, EntityId, Error, Marker, MarkerKind, Order, REPLAY_FORMAT_VERSION,
    Replay, ReplayError, ReplayHeader, Tick, World, first_divergence, run_replay,
};
use support::{flat_map, kind, pos};

const MAP_DIGEST: Digest = [0x11; 32];
const CATALOG_DIGEST: Digest = [0x22; 32];

/// A world with the two commanders a UEF and Cybran skirmish spawns, a mover
/// each, two start markers, and non-zero digests so a flipped byte differs.
fn skirmish_world() -> Arc<World> {
    let commander = |id: &str| {
        let mut k = kind(id, &["MOBILE", "COMMAND"]);
        k.motion = supercom_sim::MotionType::Land;
        k.max_speed = Some(2.0);
        k
    };
    let mut w = World::empty();
    w.units = [commander("uel0001"), commander("url0001")]
        .into_iter()
        .map(|k| (k.id.clone(), k))
        .collect();
    w.markers = vec![
        Marker {
            name: "ARMY_1".to_owned(),
            kind: MarkerKind::Army(1),
            pos: pos(4.5, 0.0, 4.5),
        },
        Marker {
            name: "ARMY_2".to_owned(),
            kind: MarkerKind::Army(2),
            pos: pos(12.5, 0.0, 12.5),
        },
    ];
    w.map = flat_map(16);
    w.map_digest = MAP_DIGEST;
    w.catalog_digest = CATALOG_DIGEST;
    Arc::new(w)
}

fn header() -> ReplayHeader {
    ReplayHeader {
        format_version: REPLAY_FORMAT_VERSION,
        engine_version: "test".to_owned(),
        map: "SYNTH".to_owned(),
        map_digest: MAP_DIGEST,
        catalog_digest: CATALOG_DIGEST,
        plugins: Vec::new(),
        players: vec![(Army(1), "UEF".to_owned()), (Army(2), "Cybran".to_owned())],
        seed: 0xDEAD_BEEF,
    }
}

fn refused(world: &Arc<World>, header: &ReplayHeader) -> Error {
    Arc::clone(world)
        .replay_start(header)
        .expect_err("the header is refused and no state exists")
}

/// DET-030: a matching header starts the skirmish at tick 0 from the
/// header's seed; each way of being wrong is refused before any state.
#[test]
fn sim_replay_start_refuses_a_wrong_header() {
    let world = skirmish_world();
    let state = Arc::clone(&world)
        .replay_start(&header())
        .expect("a matching header starts");
    assert_eq!(state.tick, Tick(0));
    assert_eq!(state.seed, 0xDEAD_BEEF);
    let armies: Vec<_> = state.entities.values().map(|e| e.army).collect();
    assert_eq!(armies, vec![Army(1), Army(2)], "one commander per player");
    assert_eq!(
        state.entities.values().next().map(|e| e.blueprint.as_str()),
        Some("uel0001")
    );
    assert_eq!(
        state.entities.values().nth(1).map(|e| e.blueprint.as_str()),
        Some("url0001")
    );

    let mismatch = Error::Replay(ReplayError::ContentMismatch);
    for byte in [0, 31] {
        let mut h = header();
        h.map_digest[byte] ^= 1;
        assert_eq!(refused(&world, &h), mismatch, "map digest byte {byte}");
        let mut h = header();
        h.catalog_digest[byte] ^= 1;
        assert_eq!(refused(&world, &h), mismatch, "catalog digest byte {byte}");
    }

    let bad = |index| Error::Replay(ReplayError::BadPlayer { index });
    for name in ["uef", "UEF ", "Terran", "", "Cybran\0"] {
        let mut h = header();
        h.players[1].1 = name.to_owned();
        assert_eq!(refused(&world, &h), bad(1), "player string {name:?}");
    }
    let mut swapped = header();
    swapped.players = vec![(Army(2), "UEF".to_owned()), (Army(1), "Cybran".to_owned())];
    assert_eq!(refused(&world, &swapped), bad(0), "slots out of order");
    let mut gap = header();
    gap.players[1].0 = Army(3);
    assert_eq!(refused(&world, &gap), bad(1), "a slot skipped");

    // The content check comes first: a header wrong both ways reports the
    // digest.
    let mut both = header();
    both.map_digest[0] ^= 1;
    both.players[0].1 = "nobody".to_owned();
    assert_eq!(refused(&world, &both), mismatch);
}

/// DET-027: the runner applies each command at its stamped tick.
#[test]
fn sim_run_replay_applies_commands_at_their_ticks() {
    let world = skirmish_world();
    let empty = Replay {
        header: header(),
        commands: Vec::new(),
        checkpoints: Vec::new(),
    };
    let mut moved = empty.clone();
    moved.commands.push(Command {
        tick: Tick(3),
        source: 0,
        seq: 0,
        army: Army(1),
        units: vec![EntityId(1)],
        order: Order::Move {
            to: pos(8.5, 0.0, 4.5),
        },
        mode: supercom_sim::QueueMode::Replace,
    });
    let (plain, _) = run_replay(Arc::clone(&world), &empty, 6).expect("an empty log runs");
    let (with, state) = run_replay(world, &moved, 6).expect("a one-command log runs");
    assert_eq!(plain.len(), 6);
    assert_eq!(with.len(), 6);
    assert_eq!(state.tick, Tick(6));
    assert_eq!(plain[..3], with[..3], "equal before the command's tick");
    assert_ne!(plain[3], with[3], "different from the command's tick");
    assert_eq!(first_divergence(&plain, &with), Some(3));
}
