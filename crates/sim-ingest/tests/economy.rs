//! Conformance of the economy and build rules against the installed game:
//! a UEF commander builds a mass extractor on a Mass marker of `SCMP_012`.
//! Game-dependent: it skips, loudly, without the install and fails if
//! `SUPERCOM_REQUIRE_GAME=1`. Counts and numbers only; no game content is
//! kept in the tree.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::sync::Arc;

use supercom_sim::{
    Army, Command, Digest, Faction, MarkerKind, Order, Pos, Q, QueueMode, State, Tick, World,
};
use supercom_sim_ingest::load_world;

/// A raw-bits division by an integer, truncating as the fixed-point type's
/// own division does: the test's own arithmetic, not the sim's helper.
fn tenth_of(per_second_raw: i64) -> i64 {
    per_second_raw / 10
}

const ONE: i64 = 1 << 32;

/// Run the scenario once, checking the stores after every tick against the
/// test's own computation, and return the digest after each tick.
fn run(world: &Arc<World>) -> Vec<Digest> {
    let mut state: State = Arc::clone(world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a UEF and Cybran skirmish");
    let marker = world
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Mass)
        .expect("a Mass marker")
        .pos;
    let commander = state
        .entities
        .values()
        .find(|e| e.army == Army(1))
        .expect("the UEF commander")
        .id;
    // Within range of the site, within 2 units of the marker.
    state
        .entities
        .get_mut(&commander)
        .expect("the commander")
        .pos = Pos {
        x: marker.x + 1.0,
        y: marker.y,
        z: marker.z,
    };
    let command = Command {
        tick: Tick(0),
        source: 0,
        seq: 0,
        army: Army(1),
        units: vec![commander],
        order: Order::BuildMobile {
            blueprint: "ueb1103".to_owned(),
            at: marker,
        },
        mode: QueueMode::Replace,
    };

    // The commander: production 1 mass and 20 energy per second, storage
    // 650 and 4000, starting full (SIM-051). The extractor: build time 60,
    // costs 36 and 360, at rate 10 that is 6 seconds, 6 mass and 60 energy
    // per second.
    let income_mass = tenth_of(ONE);
    let income_energy = tenth_of(20 * ONE);
    let request_mass = tenth_of(6 * ONE);
    let request_energy = tenth_of(60 * ONE);
    let (cap_mass, cap_energy) = (650 * ONE, 4000 * ONE);
    let (mut mass, mut energy) = (650 * ONE, 4000 * ONE);

    let mut digests = Vec::new();
    for tick in 0..=60_u64 {
        let batch = if tick == 0 {
            vec![command.clone()]
        } else {
            Vec::new()
        };
        digests.push(state.step(&batch).expect("a valid tick"));
        // The request starts the tick after the order starts the build.
        let (rm, re) = if tick == 0 {
            (0, 0)
        } else {
            (request_mass, request_energy)
        };
        mass = (mass + income_mass - rm).min(cap_mass);
        energy = (energy + income_energy - re).min(cap_energy);
        let army = state.armies.get(&Army(1)).expect("army 1");
        assert_eq!(army.mass.to_bits(), mass, "mass after tick {tick}");
        assert_eq!(army.energy.to_bits(), energy, "energy after tick {tick}");
    }
    let extractor = state
        .entities
        .values()
        .find(|e| e.blueprint == "ueb1103")
        .expect("the extractor exists");
    assert!(extractor.complete, "complete on tick 60");
    assert_eq!(state.counters.builds_completed, 1);
    assert_eq!(state.counters.stall_ticks, 0);

    // The following tick: the commander's 0.1 plus the extractor's 0.2 mass
    // (2 per second), and the extractor's maintenance 0.2 energy requested.
    digests.push(state.step(&[]).expect("a valid tick"));
    let ledger = state.armies.get(&Army(1)).expect("army 1").ledger;
    assert_eq!(
        ledger.income_mass.to_bits(),
        income_mass + tenth_of(2 * ONE)
    );
    assert_eq!(ledger.requested_energy.to_bits(), tenth_of(2 * ONE));
    assert_eq!(ledger.requested_mass, Q::ZERO);
    digests
}

#[test]
fn conformance_economy_commander_builds_extractor_on_scmp_012() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load_world(&fa, cache.path(), "SCMP_012").expect("SCMP_012 loads");
    let first = run(&world);
    let second = run(&world);
    assert_eq!(first, second, "two identical runs agree at every tick");
}
