//! Acceptance tests of the sim core. All synthetic: no install, no archive,
//! no file read. Each test is cited by a clause of docs/specs/determinism.md
//! or docs/specs/sim.md.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::BTreeSet;
use std::fmt::Write as _;

use sha2::{Digest as _, Sha256};
use supercom_sim::{
    Army, Command, Digest, Entity, EntityId, Error, GENESIS, Layer, Order, PHASES, Phase, Pos, Q,
    QueueMode, Rng, Schedule, ScheduleError, State, Tick, canonical_f64_bits, chain,
    first_divergence, run, schedule, serialize,
};

const CANON_NAN: u64 = 0x7ff8_0000_0000_0000;
const CHAIN_FIRST_HEX: &str = "e91fef5dfc74dfe92ac0921763312b19e05a97d9b87d33772c7f4826b333e561";
const CHAIN_LAST_HEX: &str = "644064e95f3f64fab2ee5d9d99802a63e308ffb84de4a8e2af1187d9f37f56db";

// ---- helpers ---------------------------------------------------------------

/// All orderings of `items` (Heap's algorithm).
fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    fn heap<T: Clone>(k: usize, a: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if k <= 1 {
            out.push(a.clone());
            return;
        }
        heap(k - 1, a, out);
        for i in 0..k - 1 {
            if k % 2 == 0 {
                a.swap(i, k - 1);
            } else {
                a.swap(0, k - 1);
            }
            heap(k - 1, a, out);
        }
    }
    let mut work = items.to_vec();
    let mut out = Vec::new();
    heap(work.len(), &mut work, &mut out);
    out
}

fn hex(d: &Digest) -> String {
    let mut s = String::with_capacity(64);
    for b in d {
        write!(s, "{b:02x}").expect("writing to a String cannot fail");
    }
    s
}

fn pos(x: f64, y: f64, z: f64) -> Pos {
    Pos { x, y, z }
}

/// A move to a point far from every spawn, so no later movement rule reaches
/// it within a test's few ticks and the order stays queued.
fn far_move(n: u32) -> Order {
    Order::Move {
        to: pos(1_000_000.0 + f64::from(n), 0.0, 0.0),
    }
}

fn command(
    (tick, source, seq): (u64, u8, u32),
    army: u8,
    unit: u32,
    order: Order,
    mode: QueueMode,
) -> Command {
    Command {
        tick: Tick(tick),
        source,
        seq,
        army: Army(army),
        units: vec![EntityId(unit)],
        order,
        mode,
    }
}

/// Six commands from four sources over three ticks, in no particular order.
/// Army 0 owns entities 1 and 2 and army 1 owns 3 and 4 in [`world`]. The
/// commands of one tick do not commute: at tick 2 sources 1's two commands on
/// unit 3 give a different queue in either order, and at tick 5 the two on
/// unit 1 do.
fn synthetic_commands() -> Vec<Command> {
    use QueueMode::{Append, Replace};
    vec![
        command((2, 1, 0), 1, 3, far_move(20), Replace),
        command((2, 0, 0), 0, 1, far_move(10), Replace),
        command((2, 1, 1), 1, 3, far_move(30), Append),
        command((0, 3, 0), 1, 4, Order::Stop, Append),
        command((5, 0, 1), 0, 1, far_move(40), Replace),
        command((5, 2, 0), 0, 1, far_move(50), Append),
    ]
}

/// Two armies and four entities: army 0 owns 1 and 2, army 1 owns 3 and 4.
fn world(seed: u64) -> State {
    let mut s = State::new(seed);
    s.add_army(
        Army(0),
        Q::from_num(1000),
        Q::from_num(5000),
        Q::from_num(4000),
        Q::from_num(8000),
    );
    s.add_army(
        Army(1),
        Q::from_num(1500),
        Q::from_num(6000),
        Q::from_num(4000),
        Q::from_num(8000),
    );
    for (army, name, x) in [(0, "a", 1.0), (0, "b", 2.0), (1, "c", 3.0), (1, "d", 4.0)] {
        s.spawn(Army(army), name, pos(x, 0.0, x), Q::from_num(100));
    }
    s
}

/// The schedule as `(tick, [(source, seq)])`, for comparison with a literal.
fn keys(s: &Schedule) -> Vec<(u64, Vec<(u8, u32)>)> {
    s.iter()
        .map(|(t, cs)| (t.0, cs.iter().map(|c| (c.source, c.seq)).collect()))
        .collect()
}

fn count(n: usize) -> u64 {
    u64::try_from(n).expect("a small count fits u64")
}

// ---- determinism: the schedule ---------------------------------------------

/// DET-002: all 720 arrival orders of six commands give one schedule.
#[test]
fn sim_schedule_ignores_arrival_order() {
    let want: Vec<(u64, Vec<(u8, u32)>)> = vec![
        (0, vec![(3, 0)]),
        (2, vec![(0, 0), (1, 0), (1, 1)]),
        (5, vec![(0, 1), (2, 0)]),
    ];
    let orders = permutations(&synthetic_commands());
    assert_eq!(orders.len(), 720);
    for order in &orders {
        let got = schedule(order, Tick(0)).expect("a valid set schedules");
        assert_eq!(keys(&got), want);
    }
}

/// DET-003: placement and count, the late boundary, duplicate refusal over
/// every arrival order, and the command phase running first.
#[test]
fn sim_schedule_applies_at_stamped_tick() {
    use QueueMode::Replace;
    let all = synthetic_commands();
    let sched = schedule(&all, Tick(0)).expect("a valid set schedules");
    let placed: usize = sched.values().map(Vec::len).sum();
    assert_eq!(placed, all.len());
    for (tick, cmds) in &sched {
        assert!(cmds.iter().all(|c| c.tick == *tick));
    }

    let late = |tick, next_tick| {
        Err(Error::Schedule(ScheduleError::Late {
            tick: Tick(tick),
            next_tick: Tick(next_tick),
        }))
    };
    // Both late and duplicate: the same refusal, late, over every order.
    let mut late_dup = synthetic_commands();
    late_dup.push(command((4, 1, 0), 1, 3, Order::Stop, Replace));
    let mut late_dup_orders = 0;
    for order in permutations(&late_dup) {
        assert_eq!(schedule(&order, Tick(3)), late(0, 3));
        late_dup_orders += 1;
    }
    assert_eq!(late_dup_orders, 5040);

    // A command stamped for the next tick is on time; one before it is late.
    assert_eq!(schedule(&all, Tick(3)), late(0, 3));
    let from_two: Vec<Command> = all.iter().filter(|c| c.tick >= Tick(2)).cloned().collect();
    assert!(schedule(&from_two, Tick(2)).is_ok());
    assert_eq!(schedule(&from_two, Tick(3)), late(2, 3));

    // Source 1 reuses sequence number 0 at another tick: refused as a
    // duplicate over every arrival order of the seven commands.
    let mut dup = synthetic_commands();
    dup.push(command((4, 1, 0), 1, 3, Order::Stop, Replace));
    let mut dup_orders = 0;
    for order in permutations(&dup) {
        assert_eq!(
            schedule(&order, Tick(0)),
            Err(Error::Schedule(ScheduleError::Duplicate {
                source: 1,
                seq: 0
            }))
        );
        dup_orders += 1;
    }
    assert_eq!(dup_orders, 5040);

    // The commands of a tick take effect before any other phase of it: the
    // traced step opens with the command phase, and the applied count
    // follows the number of on-time commands tick by tick.
    let mut state = world(7);
    let mut expected = 0;
    for t in 0..=2 {
        let batch = sched.get(&Tick(t)).cloned().unwrap_or_default();
        let (_, trace) = state.step_traced(&batch).expect("a tick steps");
        assert_eq!(trace.first(), Some(&Phase::Commands));
        expected += count(batch.len());
        assert_eq!(state.counters.commands_applied, expected);
    }
    assert_eq!(expected, 4);
}

// ---- determinism: the hash chain -------------------------------------------

/// A synthetic run of `ticks` states: integer arithmetic only, with a state
/// length that varies from 8 to 32 bytes (copied from the reference model so
/// the pinned digests carry over).
fn synthetic_states(ticks: u64) -> Vec<Vec<u8>> {
    (0..ticks)
        .map(|t| {
            let mut x = t.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let mut s = Vec::new();
            for _ in 0..=(t % 4) {
                s.extend_from_slice(&x.to_le_bytes());
                x = (x.rotate_left(13) ^ t).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            }
            s
        })
        .collect()
}

fn chain_all(states: &[Vec<u8>]) -> Vec<Digest> {
    let mut prev = GENESIS;
    let mut out = Vec::with_capacity(states.len());
    for (tick, state) in (0u64..).zip(states) {
        prev = chain(&prev, Tick(tick), state);
        out.push(prev);
    }
    out
}

/// DET-010: the committed first and last digests, and two links by hand.
#[test]
fn sim_tick_hash_chain_bits() {
    // A nonzero previous digest, a tick that differs from the length, and a
    // two-byte state, so a swapped field or a dropped byte changes the link.
    let prev: Digest = core::array::from_fn(|i| 0x5A ^ u8::try_from(i).expect("an index fits"));
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&prev);
    bytes.extend_from_slice(&3u64.to_le_bytes());
    bytes.extend_from_slice(&2u64.to_le_bytes());
    bytes.extend_from_slice(&[0xAB, 0xCD]);
    assert_eq!(
        chain(&prev, Tick(3), &[0xAB, 0xCD]),
        Digest::from(Sha256::digest(&bytes))
    );

    // A second link: another previous digest, tick 7, a five-byte state.
    let prev2: Digest = [0x11; 32];
    let mut bytes2 = prev2.to_vec();
    bytes2.extend_from_slice(&7u64.to_le_bytes());
    bytes2.extend_from_slice(&5u64.to_le_bytes());
    bytes2.extend_from_slice(&[1, 2, 3, 4, 5]);
    assert_eq!(
        chain(&prev2, Tick(7), &[1, 2, 3, 4, 5]),
        Digest::from(Sha256::digest(&bytes2))
    );

    let hashes = chain_all(&synthetic_states(16));
    assert_eq!(hashes.len(), 16);
    assert_eq!(hex(&hashes[0]), CHAIN_FIRST_HEX);
    assert_eq!(hex(&hashes[15]), CHAIN_LAST_HEX);
}

/// DET-011: a change at tick `d` leaves the prefix equal and the suffix
/// unequal, and the first unequal digest names the tick.
#[test]
fn sim_tick_hash_localizes_divergence() {
    let base = synthetic_states(40);
    let base_hashes = chain_all(&base);
    assert_eq!(first_divergence(&base_hashes, &chain_all(&base)), None);
    for d in 0..base.len() {
        let mut bent = base.clone();
        bent[d][0] ^= 1;
        let bent_hashes = chain_all(&bent);
        assert_eq!(first_divergence(&base_hashes, &bent_hashes), Some(d));
        assert_eq!(base_hashes[..d], bent_hashes[..d]);
        for t in d..base.len() {
            assert_ne!(
                base_hashes[t], bent_hashes[t],
                "tick {t} after a change at {d}"
            );
        }
    }
    assert_eq!(first_divergence(&base_hashes[..10], &base_hashes), Some(10));
}

// ---- determinism: runs, serialization, generator ---------------------------

fn digests(sched: &Schedule) -> Vec<Digest> {
    run(&mut world(7), sched, 8).expect("a run completes")
}

/// DET-013: one digest sequence for every arrival order of the schedule's
/// commands; applying them in arrival order is order sensitive (the control).
#[test]
fn sim_scheduled_run_ignores_arrival_order() {
    let mut reference: Option<Vec<Digest>> = None;
    let mut arrival_finals = BTreeSet::new();
    for order in &permutations(&synthetic_commands()) {
        let sched = schedule(order, Tick(0)).expect("a valid set schedules");
        let got = digests(&sched);
        match &reference {
            None => reference = Some(got),
            Some(first) => assert_eq!(&got, first),
        }

        // Control: the same commands grouped by tick, kept in arrival order.
        let mut naive = Schedule::new();
        for c in order {
            naive.entry(c.tick).or_default().push(c.clone());
        }
        arrival_finals.insert(digests(&naive).last().copied());
    }
    let reference = reference.expect("at least one order");
    assert_eq!(reference.len(), 8);
    assert_eq!(reference.iter().collect::<BTreeSet<_>>().len(), 8);
    assert!(
        arrival_finals.len() >= 2,
        "arrival order must change the final digest"
    );
}

fn bytes_of(state: &State) -> Vec<u8> {
    let mut out = Vec::new();
    serialize(state, &mut out);
    out
}

fn entity(id: u32, army: u8) -> Entity {
    Entity {
        id: EntityId(id),
        army: Army(army),
        blueprint: format!("bp{id}"),
        pos: pos(f64::from(id), 0.5, -f64::from(id)),
        health: Q::from_num(10),
        max_health: Q::from_num(20),
        built: Q::ZERO,
        complete: true,
        fraction: Q::ONE,
        work: None,
        heading: 0.0,
        speed: 0.0,
        layer: Layer::Land,
        route: Vec::new(),
        goal: None,
        maintenance_fraction: Q::ONE,
        orders: [Order::Stop].into(),
    }
}

/// A state with one army and one entity at `x`, for the NaN cases.
fn with_x(x: f64) -> State {
    let mut s = State::new(1);
    s.add_army(
        Army(0),
        Q::from_num(1),
        Q::from_num(2),
        Q::from_num(3),
        Q::from_num(4),
    );
    s.spawn(Army(0), "n", pos(x, 0.0, 0.0), Q::from_num(5));
    s
}

/// DET-009: equal states give equal bytes whatever the insertion order, a NaN
/// is written as the canonical pattern, and a minimal state matches the
/// documented field order byte for byte.
#[test]
fn sim_serialization_is_canonical() {
    // Same armies and entities, inserted in opposite orders.
    let (mut a, mut b) = (State::new(1), State::new(1));
    for id in [1, 2, 3] {
        a.add_army(Army(id), Q::from_num(id), Q::ZERO, Q::ZERO, Q::ZERO);
        a.entities
            .insert(EntityId(10 + u32::from(id)), entity(10 + u32::from(id), id));
    }
    for id in [3, 2, 1] {
        b.add_army(Army(id), Q::from_num(id), Q::ZERO, Q::ZERO, Q::ZERO);
        b.entities
            .insert(EntityId(10 + u32::from(id)), entity(10 + u32::from(id), id));
    }
    a.next_id = EntityId(20);
    b.next_id = EntityId(20);
    let (sa, sb) = (bytes_of(&a), bytes_of(&b));
    assert!(!sa.is_empty(), "a state serializes to bytes");
    assert_eq!(sa, sb);

    // NaNs of either sign and several payloads serialize canonically.
    for bits in [
        f64::NAN.to_bits(),
        (-f64::NAN).to_bits(),
        0x7ff0_0000_0000_0001,
        0xfff8_dead_beef_0001,
    ] {
        let nan = f64::from_bits(bits);
        assert!(nan.is_nan());
        assert_eq!(canonical_f64_bits(nan), CANON_NAN);
        let got = bytes_of(&with_x(nan));
        let canon = bytes_of(&with_x(f64::from_bits(CANON_NAN)));
        assert_eq!(got, canon, "NaN bits {bits:#018x}");
        assert!(got.windows(8).any(|w| w == CANON_NAN.to_le_bytes()));
        if bits != CANON_NAN {
            assert!(!got.windows(8).any(|w| w == bits.to_le_bytes()));
        }
    }
    for x in [1.5, -0.0, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(canonical_f64_bits(x), x.to_bits());
    }

    // A minimal state against the documented field order: tick, seed, the
    // generator state, the next id, the two counters, the armies (count, then
    // id, mass, energy, mass storage, energy storage as raw i64 bits, the
    // defeated flag), then the entities (count, then id, army, blueprint
    // length and bytes, x, y, z, health, max health, built, complete,
    // fraction, work, heading, speed, layer, the route, the goal, the
    // maintenance fraction, and the queued orders, counted, in the order
    // encoding). The counters carry builds completed, stall ticks, and moves
    // completed, and each army its ledger of eight values.
    let mut s = State::new(5);
    s.add_army(
        Army(1),
        Q::from_num(3),
        Q::from_num(4),
        Q::from_num(5),
        Q::from_num(6),
    );
    let id = s.spawn(Army(1), "ab", pos(1.5, 2.0, -0.5), Q::from_num(10));
    s.entities
        .get_mut(&id)
        .expect("the spawned entity exists")
        .orders
        .push_back(Order::Stop);
    let mut want = Vec::new();
    want.extend_from_slice(&0u64.to_le_bytes()); // tick
    want.extend_from_slice(&5u64.to_le_bytes()); // seed
    want.extend_from_slice(&s.rng.state().to_le_bytes());
    want.extend_from_slice(&2u32.to_le_bytes()); // next id
    want.extend_from_slice(&0u64.to_le_bytes()); // commands applied
    want.extend_from_slice(&0u64.to_le_bytes()); // commands ignored
    want.extend_from_slice(&0u64.to_le_bytes()); // builds completed
    want.extend_from_slice(&0u64.to_le_bytes()); // stall ticks
    want.extend_from_slice(&0u64.to_le_bytes()); // moves completed
    want.extend_from_slice(&1u32.to_le_bytes()); // one army
    want.push(1);
    for v in [3i64, 4, 5, 6] {
        want.extend_from_slice(&(v << 32).to_le_bytes());
    }
    want.push(0); // not defeated
    for v in [0i64, 0, 0, 0, 0, 0, 1, 1] {
        want.extend_from_slice(&(v << 32).to_le_bytes()); // the ledger
    }
    want.extend_from_slice(&1u32.to_le_bytes()); // one entity
    want.extend_from_slice(&1u32.to_le_bytes()); // id
    want.push(1); // army
    want.extend_from_slice(&2u32.to_le_bytes());
    want.extend_from_slice(b"ab");
    for v in [1.5f64, 2.0, -0.5] {
        want.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    want.extend_from_slice(&(10i64 << 32).to_le_bytes()); // health
    want.extend_from_slice(&(10i64 << 32).to_le_bytes()); // max health
    want.extend_from_slice(&0i64.to_le_bytes()); // built
    want.push(1); // complete
    want.extend_from_slice(&(1i64 << 32).to_le_bytes()); // fraction
    want.push(0); // no work
    want.extend_from_slice(&0u64.to_le_bytes()); // heading 0.0
    want.extend_from_slice(&0u64.to_le_bytes()); // speed 0.0
    want.push(1); // layer Land, declaration index 1
    want.extend_from_slice(&0u32.to_le_bytes()); // empty route
    want.push(0); // no goal
    want.extend_from_slice(&(1i64 << 32).to_le_bytes()); // maintenance fraction
    want.extend_from_slice(&1u32.to_le_bytes()); // one queued order
    want.push(1); // Stop
    assert_eq!(bytes_of(&s), want);
}

fn outputs(seed: u64) -> Vec<u64> {
    let mut rng = Rng::from_seed(seed);
    (0..64).map(|_| rng.next_u64()).collect()
}

/// DET-025: the generator is a function of its seed, and a state's digests
/// follow from its seed and the schedule alone.
#[test]
fn sim_rng_is_seeded_from_initial_state() {
    let first = outputs(1);
    assert!(
        first.iter().collect::<BTreeSet<_>>().len() > 1,
        "the generator must not be constant"
    );
    assert_eq!(first, outputs(1));
    assert_ne!(first, outputs(2));

    let sched = schedule(&synthetic_commands(), Tick(0)).expect("a valid set schedules");
    let run_of = |seed| run(&mut world(seed), &sched, 8).expect("a run completes");
    assert_eq!(run_of(7), run_of(7));
    assert_ne!(run_of(7), run_of(8));
}

// ---- sim: tick loop, commands, snapshot ------------------------------------

/// SIM-044: the phase constant and one step's trace are the stated order.
#[test]
fn sim_tick_phases_in_order() {
    assert_eq!(
        PHASES,
        [
            Phase::Commands,
            Phase::Economy,
            Phase::Build,
            Phase::Movement,
            Phase::Weapons,
            Phase::Projectiles,
            Phase::Damage,
            Phase::Death,
            Phase::Intel,
            Phase::Victory,
        ]
    );
    let mut state = world(7);
    let (_, trace) = state.step_traced(&[]).expect("an empty tick steps");
    assert_eq!(trace, PHASES);
    assert_eq!(state.tick, Tick(1));
}

/// SIM-045: ownership, counters, and the replace and append semantics.
#[test]
fn sim_command_application_and_ownership() {
    use QueueMode::{Append, Replace};
    let mut state = world(7);
    let (a, b, c) = (far_move(1), far_move(2), far_move(3));
    state
        .step(&[
            command((0, 0, 0), 0, 1, a.clone(), Append),
            command((0, 0, 1), 0, 1, b.clone(), Append),
            // Army 1 does not own entity 1: ignored, and it must not clear
            // the queue although it replaces.
            command((0, 1, 0), 1, 1, far_move(9), Replace),
        ])
        .expect("a tick steps");
    let queue = |s: &State| {
        s.entities[&EntityId(1)]
            .orders
            .iter()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(queue(&state), vec![a, b]);
    assert_eq!(state.counters.commands_applied, 2);
    assert_eq!(state.counters.commands_ignored, 1);

    state
        .step(&[command((1, 0, 2), 0, 1, c.clone(), Replace)])
        .expect("a tick steps");
    assert_eq!(queue(&state), vec![c]);
    assert_eq!(state.counters.commands_applied, 3);
    assert_eq!(state.counters.commands_ignored, 1);
}

/// SIM-046: a snapshot lists entities and armies ascending, with the state's
/// values.
#[test]
fn sim_snapshot_lists_entities_in_id_order() {
    let mut state = State::new(3);
    for (id, mass) in [(2u8, 20), (0, 0), (1, 10)] {
        state.add_army(
            Army(id),
            Q::from_num(mass),
            Q::from_num(mass + 1),
            Q::from_num(100),
            Q::from_num(200),
        );
    }
    state
        .armies
        .get_mut(&Army(1))
        .expect("army 1 exists")
        .defeated = true;
    for (army, x) in [(2, 5.0), (0, 1.0), (1, 3.0)] {
        state.spawn(Army(army), "u", pos(x, 0.25, -x), Q::from_num(30));
    }
    // An entity inserted out of spawn order, with a lower id.
    state.entities.insert(EntityId(0), entity(0, 1));

    let snap = state.snapshot();
    assert_eq!(snap.tick, state.tick);
    assert_eq!(snap.entities.len(), 4);
    assert_eq!(snap.armies.len(), 3);
    assert!(snap.entities.windows(2).all(|w| w[0].id < w[1].id));
    assert!(snap.armies.windows(2).all(|w| w[0].id < w[1].id));
    for (snap_e, e) in snap.entities.iter().zip(state.entities.values()) {
        assert_eq!(snap_e.id, e.id);
        assert_eq!(snap_e.army, e.army);
        assert_eq!(snap_e.pos, e.pos);
        assert_eq!(snap_e.health, e.health);
        assert_eq!(snap_e.max_health, e.max_health);
    }
    for (snap_a, a) in snap.armies.iter().zip(state.armies.values()) {
        assert_eq!(snap_a.id, a.id);
        assert_eq!(snap_a.mass, a.mass);
        assert_eq!(snap_a.energy, a.energy);
        assert_eq!(snap_a.defeated, a.defeated);
    }
}
