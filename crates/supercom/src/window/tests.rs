use super::*;
use crate::cache::CacheVars;
use crate::pacing::Pace;
use crate::prepare::prepare_replay;
use crate::testing::{Scratch, committed_log};
use std::sync::Arc;
use supercom_sim::{Army, Tick, World, first_divergence, read_replay, run_replay as sim_replay};

fn pace_of(due: u64, held_back: u64) -> Pace {
    Pace {
        due,
        held_back,
        fraction: 0.0,
    }
}

/// A run over an empty world whose checkpoints come from stepping a copy of
/// the state directly.
fn empty_run() -> (Play, Vec<Digest>) {
    let state = State::with_world(Arc::new(World::empty()), 3);
    let mut oracle = state.clone();
    let mut digests = Vec::new();
    for _ in 0..6 {
        digests.push(oracle.step(&[]).unwrap_or([0; 32]));
    }
    let replay = Replay {
        header: supercom_sim::ReplayHeader {
            format_version: supercom_sim::REPLAY_FORMAT_VERSION,
            engine_version: "test".to_owned(),
            map: "m".to_owned(),
            map_digest: [0; 32],
            catalog_digest: [0; 32],
            plugins: Vec::new(),
            players: vec![(Army(1), "UEF".to_owned())],
            seed: 3,
        },
        commands: Vec::new(),
        checkpoints: vec![(Tick(2), digests[2]), (Tick(5), digests[5])],
    };
    (Play::new(state, replay), digests)
}

/// Frames of 3 due: the counts after each, the held-back frame counted once
/// and not after stepping has stopped, and the stop after tick 5.
#[test]
fn supercom_play_counts_held_frames_and_stops_at_the_last_checkpoint() {
    let (mut play, _) = empty_run();
    assert_eq!((play.ticks_run(), play.matched(), play.total()), (0, 0, 2));
    assert!(!play.finished());

    assert!(play.frame(&pace_of(3, 2)).is_ok());
    assert_eq!(
        (play.ticks_run(), play.matched(), play.held_frames()),
        (3, 1, 1),
        "ticks 0 to 2 ran and tick 2's checkpoint matched"
    );
    assert!(!play.finished());

    assert!(play.frame(&pace_of(3, 0)).is_ok());
    assert_eq!(
        (play.ticks_run(), play.matched(), play.held_frames()),
        (6, 2, 1),
        "ticks 3 to 5 ran; a frame that held nothing back is not counted"
    );
    assert!(play.finished());

    assert!(play.frame(&pace_of(3, 2)).is_ok());
    assert_eq!(
        (play.ticks_run(), play.matched(), play.held_frames()),
        (6, 2, 1),
        "nothing is stepped or counted once stepping has stopped"
    );
    assert_eq!(play.step_one(), Ok(Stepped::Stopped));
}

/// Against the install: 4,600 steps, all 46 checkpoints matched, and the
/// digests equal those of the simulation's own run.
#[test]
fn conformance_replay_loop_matches_the_committed_log() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let root = Scratch::new("play");
    let prepared = prepare_replay(Some(&fa), &committed_log(), root.cache_vars());
    assert!(prepared.is_ok(), "{:?}", prepared.as_ref().err());
    let Ok(prepared) = prepared else { return };
    let world = Arc::clone(prepared.state.world());
    let replay = prepared.replay.clone();
    let oracle = sim_replay(world, &replay, 4600);
    assert!(oracle.is_ok(), "{:?}", oracle.as_ref().err());
    let Ok((expected, _)) = oracle else { return };

    let mut play = Play::new(prepared.state, prepared.replay);
    let mut digests = Vec::new();
    loop {
        let stepped = play.step_one();
        assert!(
            stepped.is_ok(),
            "the loop failed at step {}: {:?}",
            digests.len(),
            stepped.as_ref().err()
        );
        match stepped {
            Ok(Stepped::Digest(digest)) => digests.push(digest),
            Ok(Stepped::Stopped) | Err(_) => break,
        }
        assert!(digests.len() <= 4600, "the loop never stops");
    }
    assert_eq!(digests.len(), 4600);
    assert_eq!(play.ticks_run(), 4600);
    assert_eq!((play.matched(), play.total()), (46, 46));
    assert_eq!(first_divergence(&digests, &expected), None);
    assert!(play.finished());
}

/// A wrong digest at tick 99 stops the loop at the hundredth step.
#[test]
fn conformance_replay_loop_names_the_tick_of_a_wrong_checkpoint() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let root = Scratch::new("play-wrong");
    let prepared = prepare_replay(Some(&fa), &committed_log(), root.cache_vars());
    assert!(prepared.is_ok(), "{:?}", prepared.as_ref().err());
    let Ok(mut prepared) = prepared else { return };
    assert_eq!(
        prepared.replay.checkpoints.first().map(|c| c.0),
        Some(Tick(99))
    );
    if let Some(first) = prepared.replay.checkpoints.first_mut() {
        first.1[0] ^= 1;
    }
    let mut play = Play::new(prepared.state, prepared.replay);
    for step in 0..99 {
        assert!(
            matches!(play.step_one(), Ok(Stepped::Digest(_))),
            "step {step} runs"
        );
    }
    assert_eq!(play.step_one(), Err(RunError::Mismatch { tick: 99 }));
    assert_eq!((play.ticks_run(), play.matched()), (100, 0));
    let _ = (CacheVars::default(), read_replay(&[]));
}
