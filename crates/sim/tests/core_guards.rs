//! Guards on the core's behaviour that the frozen acceptance tests in
//! `sim.rs` leave open: what `step` refuses, the generator's identity, and the
//! composition of the serialized state and the hash chain. All synthetic.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_sim::{
    Army, Command, EntityId, Error, GENESIS, Order, Pos, Q, QueueMode, Rng, ScheduleError, State,
    Tick, chain, serialize,
};

fn move_to(n: u32) -> Order {
    Order::Move {
        to: Pos {
            x: 1_000_000.0 + f64::from(n),
            y: 0.0,
            z: 0.0,
        },
    }
}

fn command(
    (tick, source, seq): (u64, u8, u32),
    army: u8,
    units: &[u32],
    order: Order,
    mode: QueueMode,
) -> Command {
    Command {
        tick: Tick(tick),
        source,
        seq,
        army: Army(army),
        units: units.iter().copied().map(EntityId).collect(),
        order,
        mode,
    }
}

/// Army 0 owns entities 1 and 2, army 1 owns 3 and 4.
fn world(seed: u64) -> State {
    let mut s = State::new(seed);
    for army in [0, 1] {
        s.add_army(
            Army(army),
            Q::from_num(1000),
            Q::from_num(5000),
            Q::from_num(4000),
            Q::from_num(8000),
        );
    }
    for (army, name, x) in [(0, "a", 1.0), (0, "b", 2.0), (1, "c", 3.0), (1, "d", 4.0)] {
        s.spawn(Army(army), name, Pos { x, y: 0.0, z: x }, Q::from_num(100));
    }
    s
}

/// Run `batch` through both entry points and check each refuses with `want`
/// and leaves the state equal to its clone from before the call.
fn assert_refused(batch: &[Command], want: &Error) {
    let mut s = world(0);
    let before = s.clone();
    assert_eq!(s.step(batch).as_ref(), Err(want));
    assert_eq!(s, before, "step changed the state it refused");
    assert_eq!(
        s.step_traced(batch).as_ref().map(|(d, _)| d),
        Err(want),
        "step_traced must refuse the same batch"
    );
    assert_eq!(s, before, "step_traced changed the state it refused");
}

#[test]
fn step_refuses_a_command_stamped_for_another_tick() {
    use QueueMode::Replace;
    // A valid command first, so a refusal that only checks the first command,
    // or checks after applying, still shows in the state.
    let batch = [
        command((0, 0, 0), 0, &[1], move_to(1), Replace),
        command((5, 0, 1), 0, &[1], move_to(2), Replace),
    ];
    assert_refused(
        &batch,
        &Error::WrongTick {
            tick: Tick(5),
            expected: Tick(0),
        },
    );
}

#[test]
fn step_refuses_a_source_and_seq_repeated_in_one_batch() {
    use QueueMode::Replace;
    let batch = [
        command((0, 1, 7), 0, &[1], move_to(1), Replace),
        command((0, 1, 7), 0, &[2], move_to(2), Replace),
    ];
    assert_refused(
        &batch,
        &Error::Schedule(ScheduleError::Duplicate { source: 1, seq: 7 }),
    );
}

/// Vigna's splitmix64.c for seed 0: the state advances by the increment, then
/// the advanced state is mixed. Values are the reference's, not the crate's.
#[test]
fn sim_rng_matches_the_reference_splitmix64() {
    let mut rng = Rng::from_seed(0);
    let got = [rng.next_u64(), rng.next_u64(), rng.next_u64()];
    assert_eq!(
        got,
        [
            0xe220_a839_7b1d_cdaf,
            0x6e78_9e6a_a1b9_65f4,
            0x06c4_5d18_8009_454f
        ]
    );
}

/// A golden vector for a second seed, as the crate yields it at the commit
/// that added this test; it pins the generator against any change of constant
/// or shift.
#[test]
fn sim_rng_golden_vector_for_seed_1234567() {
    let mut rng = Rng::from_seed(1_234_567);
    let got: Vec<u64> = (0..5).map(|_| rng.next_u64()).collect();
    assert_eq!(got, GOLDEN);
}

const GOLDEN: [u64; 5] = [
    6_457_827_717_110_365_317,
    3_203_168_211_198_807_973,
    9_817_491_932_198_370_423,
    4_593_380_528_125_082_431,
    16_408_922_859_458_223_821,
];

/// S carries the generator's state at bytes 16 to 23, after tick and seed
/// (DET-009); the literal is seed 5 plus one increment, little endian.
#[test]
fn sim_serialized_generator_field_is_the_state_after_a_draw() {
    let mut s = State::new(5);
    let _ = s.rng.next_u64();
    let mut bytes = Vec::new();
    serialize(&s, &mut bytes);
    assert_eq!(bytes[16..24], 0x9E37_79B9_7F4A_7C1A_u64.to_le_bytes());
}

#[test]
fn sim_mixed_ownership_command_is_ignored_whole() {
    let mut s = world(0);
    let mixed = command((0, 0, 0), 0, &[1, 3], move_to(1), QueueMode::Replace);
    let before = s.entities.clone();
    s.step(&[mixed]).expect("a well-formed batch");
    assert_eq!(s.entities, before, "no queue may change");
    assert_eq!(s.counters.commands_ignored, 1);
    assert_eq!(s.counters.commands_applied, 0);
}

/// Each digest is `chain(prev, t, S)` for the tick `t` just run and S
/// serialized from the state after the step (DET-010).
#[test]
fn sim_step_digest_is_chain_over_the_end_of_tick_state() {
    let mut s = world(3);
    let mut prev = GENESIS;
    for t in 0..2 {
        let digest = s.step(&[]).expect("an empty batch");
        let mut bytes = Vec::new();
        serialize(&s, &mut bytes);
        assert_eq!(digest, chain(&prev, Tick(t), &bytes), "tick {t}");
        prev = digest;
    }
}
