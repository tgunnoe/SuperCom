//! Guards on the core's behaviour that the frozen acceptance tests in
//! `sim.rs` leave open: what `step` refuses, the generator's identity, and the
//! composition of the serialized state and the hash chain. All synthetic.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_sim::{
    Army, Command, EntityId, Error, Order, Pos, Q, QueueMode, ScheduleError, State, Tick,
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
