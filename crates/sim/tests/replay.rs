//! Acceptance tests of the replay format (docs/specs/determinism.md, DET-007,
//! DET-008, DET-026). All synthetic: the fixture is built here by a reference
//! encoder that shares no code with the crate's writer.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_sim::{
    Army, Command, Digest, EntityId, Error, Order, Pos, Q, QueueMode, REPLAY_FORMAT_VERSION,
    Replay, ReplayError, ReplayHeader, State, Tick, check_content, commands_at, read_replay,
    write_replay,
};

/// The committed golden vector of DET-026, read at compile time so the test
/// opens no file.
const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/synthetic.replay"
));

// ---- the reference encoder (DET-026), written from the layout alone ---------

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_len(out: &mut Vec<u8>, n: usize) {
    put_u32(out, u32::try_from(n).expect("a synthetic length fits u32"));
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_len(out, s.len());
    out.extend_from_slice(s.as_bytes());
}

fn put_f64(out: &mut Vec<u8>, v: f64) {
    // The synthetic values are finite, so the raw bits are the canonical bits.
    put_u64(out, v.to_bits());
}

fn put_pos(out: &mut Vec<u8>, p: &Pos) {
    put_f64(out, p.x);
    put_f64(out, p.y);
    put_f64(out, p.z);
}

fn put_order(out: &mut Vec<u8>, order: &Order) {
    match order {
        Order::Move { to } => {
            out.push(0);
            put_pos(out, to);
        }
        Order::Stop => out.push(1),
        Order::Attack { target } => {
            out.push(2);
            put_u32(out, target.0);
        }
        Order::BuildMobile { blueprint, at } => {
            out.push(3);
            put_str(out, blueprint);
            put_pos(out, at);
        }
        Order::BuildFactory { blueprint } => {
            out.push(4);
            put_str(out, blueprint);
        }
        Order::Guard { target } => {
            out.push(5);
            put_u32(out, target.0);
        }
    }
}

fn put_command(out: &mut Vec<u8>, c: &Command) {
    put_u64(out, c.tick.0);
    out.push(c.source);
    put_u32(out, c.seq);
    out.push(c.army.0);
    put_len(out, c.units.len());
    for id in &c.units {
        put_u32(out, id.0);
    }
    out.push(match c.mode {
        QueueMode::Replace => 0,
        QueueMode::Append => 1,
    });
    put_order(out, &c.order);
}

/// The bytes of `r`, whatever its content: the encoder validates nothing, so
/// the malformed-input test builds its bad files through it.
fn reference_encode(r: &Replay) -> Vec<u8> {
    let h = &r.header;
    let mut out = Vec::new();
    out.extend_from_slice(b"SCREPLAY");
    put_u32(&mut out, h.format_version);
    put_str(&mut out, &h.engine_version);
    put_str(&mut out, &h.map);
    out.extend_from_slice(&h.map_digest);
    out.extend_from_slice(&h.catalog_digest);
    put_len(&mut out, h.plugins.len());
    for (name, digest) in &h.plugins {
        put_str(&mut out, name);
        out.extend_from_slice(digest);
    }
    put_len(&mut out, h.players.len());
    for (army, name) in &h.players {
        out.push(army.0);
        put_str(&mut out, name);
    }
    put_u64(&mut out, h.seed);
    put_len(&mut out, r.commands.len());
    for c in &r.commands {
        put_command(&mut out, c);
    }
    put_len(&mut out, r.checkpoints.len());
    for (tick, digest) in &r.checkpoints {
        put_u64(&mut out, tick.0);
        out.extend_from_slice(digest);
    }
    out
}

// ---- the synthetic replay ----------------------------------------------------

/// A digest whose bytes ascend from `start`, so a swapped field is visible.
fn digest(start: u8) -> Digest {
    core::array::from_fn(|i| start.wrapping_add(u8::try_from(i).expect("an index fits")))
}

fn pos(x: f64, y: f64, z: f64) -> Pos {
    Pos { x, y, z }
}

fn command(
    tick: u64,
    source: u8,
    seq: u32,
    army: u8,
    units: &[u32],
    mode: QueueMode,
    order: Order,
) -> Command {
    Command {
        tick: Tick(tick),
        source,
        seq,
        army: Army(army),
        units: units.iter().map(|&u| EntityId(u)).collect(),
        order,
        mode,
    }
}

/// Two armies, four commands, three checkpoints, no plugins (v0 runs none).
fn synthetic() -> Replay {
    Replay {
        header: ReplayHeader {
            format_version: REPLAY_FORMAT_VERSION,
            engine_version: "supercom-sim 0.0.1".to_owned(),
            map: "synthetic/duel".to_owned(),
            map_digest: digest(0x10),
            catalog_digest: digest(0x40),
            plugins: Vec::new(),
            players: vec![(Army(0), "alpha".to_owned()), (Army(1), "beta".to_owned())],
            seed: 0x0123_4567_89AB_CDEF,
        },
        commands: vec![
            command(
                0,
                0,
                0,
                0,
                &[1, 2],
                QueueMode::Replace,
                Order::Move {
                    to: pos(1.5, 0.0, -2.25),
                },
            ),
            command(
                0,
                1,
                0,
                1,
                &[3],
                QueueMode::Append,
                Order::BuildFactory {
                    blueprint: "synthetic_factory".to_owned(),
                },
            ),
            command(
                3,
                0,
                1,
                0,
                &[1],
                QueueMode::Replace,
                Order::Attack {
                    target: EntityId(3),
                },
            ),
            command(
                5,
                1,
                1,
                1,
                &[3, 4],
                QueueMode::Replace,
                Order::BuildMobile {
                    blueprint: "synthetic_tank".to_owned(),
                    at: pos(8.0, 0.5, 9.0),
                },
            ),
        ],
        checkpoints: vec![
            (Tick(2), digest(0x70)),
            (Tick(4), digest(0xA0)),
            (Tick(6), digest(0xD0)),
        ],
    }
}

/// No players, no commands, no checkpoints.
fn empty() -> Replay {
    Replay {
        header: ReplayHeader {
            format_version: REPLAY_FORMAT_VERSION,
            engine_version: String::new(),
            map: String::new(),
            map_digest: digest(0),
            catalog_digest: digest(1),
            plugins: Vec::new(),
            players: Vec::new(),
            seed: 0,
        },
        commands: Vec::new(),
        checkpoints: Vec::new(),
    }
}

/// All six orders, and a plugin entry the v0 writer never emits but the
/// reader must accept (DET-007).
fn wide() -> Replay {
    let mut r = synthetic();
    r.header.plugins = vec![
        ("first".to_owned(), digest(0x20)),
        (String::new(), digest(0x21)),
    ];
    r.commands = vec![
        command(1, 0, 0, 0, &[], QueueMode::Replace, Order::Stop),
        command(
            1,
            0,
            1,
            0,
            &[7],
            QueueMode::Append,
            Order::Guard {
                target: EntityId(9),
            },
        ),
        command(
            2,
            0,
            2,
            0,
            &[u32::MAX],
            QueueMode::Replace,
            Order::Attack {
                target: EntityId(0),
            },
        ),
        command(
            2,
            1,
            0,
            1,
            &[1, 2, 3],
            QueueMode::Append,
            Order::Move {
                to: pos(-0.0, f64::MAX, f64::MIN_POSITIVE),
            },
        ),
        command(
            9,
            1,
            1,
            1,
            &[4],
            QueueMode::Replace,
            Order::BuildFactory {
                blueprint: "caf\u{e9}".to_owned(),
            },
        ),
        command(
            9,
            1,
            2,
            1,
            &[5],
            QueueMode::Replace,
            Order::BuildMobile {
                blueprint: String::new(),
                at: pos(0.0, 0.0, 0.0),
            },
        ),
    ];
    r
}

/// What the reader makes of the reference encoder's bytes for `r`.
fn refuses(r: &Replay) -> Result<Replay, Error> {
    read_replay(&reference_encode(r))
}

fn written(r: &Replay) -> Vec<u8> {
    let mut out = Vec::new();
    write_replay(r, &mut out);
    out
}

/// Writes `fixtures/synthetic.replay` from the reference encoder. Run once,
/// by hand, after the layout or the synthetic replay changes:
/// `cargo test -p supercom-sim --test replay -- --ignored regenerate`.
#[test]
#[ignore = "writes the committed fixture; run by hand"]
#[expect(
    clippy::disallowed_methods,
    reason = "the sim bans filesystem writes; this test-only generator is the one writer of the fixture"
)]
fn regenerate_synthetic_fixture() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/synthetic.replay");
    std::fs::write(path, reference_encode(&synthetic())).expect("the fixture is writable");
}

// ---- the tests -----------------------------------------------------------------

/// DET-007, DET-008: what the writer writes the reader reads back, the content
/// check refuses a different install, and the log is searched by stamp.
#[test]
fn sim_replay_round_trip() {
    for r in [synthetic(), empty(), wide()] {
        assert_eq!(read_replay(&written(&r)), Ok(r.clone()));
    }

    let h = synthetic().header;
    assert_eq!(check_content(&h, &h.map_digest, &h.catalog_digest), Ok(()));
    let other = digest(0xEE);
    assert_eq!(
        check_content(&h, &other, &h.catalog_digest),
        Err(Error::Replay(ReplayError::ContentMismatch)),
    );
    assert_eq!(
        check_content(&h, &h.map_digest, &other),
        Err(Error::Replay(ReplayError::ContentMismatch)),
    );

    let r = synthetic();
    assert_eq!(commands_at(&r, Tick(0)), &r.commands[0..2]);
    assert_eq!(commands_at(&r, Tick(3)), &r.commands[2..3]);
    assert_eq!(commands_at(&r, Tick(5)), &r.commands[3..4]);
    for quiet in [1, 2, 4, 6, u64::MAX] {
        assert!(commands_at(&r, Tick(quiet)).is_empty(), "tick {quiet}");
    }
    assert_eq!(commands_at(&empty(), Tick(0)), []);
}

/// DET-007, DET-008, DET-026: each way a byte string can be wrong has its own
/// refusal, and no length reaches an allocation before it is checked.
#[test]
fn sim_replay_rejects_malformed() {
    let good = reference_encode(&synthetic());
    for cut in 0..good.len() {
        assert!(
            matches!(
                read_replay(&good[..cut]),
                Err(Error::Replay(
                    ReplayError::Truncated | ReplayError::LengthExceedsInput { .. }
                ))
            ),
            "a replay cut at byte {cut} of {}",
            good.len(),
        );
    }

    let mut bad_magic = good.clone();
    bad_magic[0] = b'X';
    assert_eq!(
        read_replay(&bad_magic),
        Err(Error::Replay(ReplayError::BadMagic))
    );

    let mut v2 = synthetic();
    v2.header.format_version = 2;
    assert_eq!(
        refuses(&v2),
        Err(Error::Replay(ReplayError::UnsupportedVersion(2)))
    );

    // Same tick, source 1 before source 0; then a later tick before an earlier.
    let mut same_tick = synthetic();
    same_tick.commands.swap(0, 1);
    assert_eq!(
        refuses(&same_tick),
        Err(Error::Replay(ReplayError::RecordsOutOfOrder { index: 1 }))
    );
    let mut later_first = synthetic();
    later_first.commands.swap(2, 3);
    assert_eq!(
        refuses(&later_first),
        Err(Error::Replay(ReplayError::RecordsOutOfOrder { index: 3 }))
    );

    // (source 0, seq 0) again at tick 3: in schedule order, yet a repeat.
    let mut repeat = synthetic();
    repeat.commands[2].seq = 0;
    assert_eq!(
        refuses(&repeat),
        Err(Error::Replay(ReplayError::DuplicateRecord { index: 2 }))
    );

    // Source 0 holds slot 0, army 0; the record names army 1.
    let mut foreign = synthetic();
    foreign.commands[2].army = Army(1);
    assert_eq!(
        refuses(&foreign),
        Err(Error::Replay(ReplayError::ArmyNotInSlot { index: 2 }))
    );

    let mut equal = synthetic();
    equal.checkpoints[2].0 = Tick(4);
    assert_eq!(
        refuses(&equal),
        Err(Error::Replay(ReplayError::CheckpointsNotAscending {
            index: 2
        }))
    );
    let mut descending = synthetic();
    descending.checkpoints.swap(0, 1);
    assert_eq!(
        refuses(&descending),
        Err(Error::Replay(ReplayError::CheckpointsNotAscending {
            index: 1
        }))
    );

    // One Stop command and no checkpoints: the file ends mode, tag, 0u32.
    let mut one = empty();
    one.header.players = vec![(Army(0), String::new())];
    one.commands = vec![command(0, 0, 0, 0, &[], QueueMode::Replace, Order::Stop)];
    let bytes = reference_encode(&one);
    let tag = bytes.len() - 5;
    assert_eq!((bytes[tag - 1], bytes[tag]), (0, 1), "the layout moved");
    let mut bad_tag = bytes.clone();
    bad_tag[tag] = 9;
    assert_eq!(
        read_replay(&bad_tag),
        Err(Error::Replay(ReplayError::BadOrderTag(9)))
    );
    let mut bad_mode = bytes;
    bad_mode[tag - 1] = 7;
    assert_eq!(
        read_replay(&bad_mode),
        Err(Error::Replay(ReplayError::BadMode(7)))
    );

    // The engine version's first byte follows magic, version, and its length.
    let mut bad_string = good;
    bad_string[16] = 0xFF;
    assert_eq!(
        read_replay(&bad_string),
        Err(Error::Replay(ReplayError::BadString))
    );

    hostile_lengths_are_refused();
}

/// A length is checked against the remaining input before it is trusted.
fn hostile_lengths_are_refused() {
    // Magic, version, then a string length of u32::MAX in a 16-byte input.
    let mut hostile = Vec::from(*b"SCREPLAY");
    hostile.extend_from_slice(&REPLAY_FORMAT_VERSION.to_le_bytes());
    hostile.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(hostile.len(), 16);
    assert!(matches!(
        read_replay(&hostile),
        Err(Error::Replay(ReplayError::LengthExceedsInput { .. }))
    ));

    // The two trailing counts of an empty replay claim u32::MAX entries.
    let tail = reference_encode(&empty());
    for count_at in [tail.len() - 8, tail.len() - 4] {
        let mut huge = tail.clone();
        huge[count_at..count_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(
            matches!(
                read_replay(&huge),
                Err(Error::Replay(ReplayError::LengthExceedsInput { .. }))
            ),
            "a count at byte {count_at}"
        );
    }
}

/// DET-026: the committed fixture is the reference encoder's output, the
/// crate's writer reproduces it byte for byte, and the reader parses it back.
#[test]
fn sim_replay_fixture_is_byte_exact() {
    let r = synthetic();
    assert!(FIXTURE.len() <= 4096, "{} bytes", FIXTURE.len());
    assert_eq!(
        reference_encode(&r),
        FIXTURE,
        "the fixture is stale: regenerate it"
    );
    assert_eq!(written(&r), FIXTURE);
    assert_eq!(read_replay(FIXTURE), Ok(r));
}

fn resumable_world() -> State {
    let mut s = State::new(0xFACE);
    s.add_army(
        Army(0),
        Q::from_num(100),
        Q::from_num(200),
        Q::from_num(300),
        Q::from_num(400),
    );
    s.spawn(Army(0), "unit", pos(1.0, 0.0, 2.0), Q::from_num(50));
    s
}

/// DET-007, DET-010: the chain head lies outside S, so a run resumed from a
/// checkpoint continues the chain only by taking the checkpoint's digest.
#[test]
fn sim_run_resumes_from_a_checkpoint_head() {
    let second = [command(
        1,
        0,
        0,
        0,
        &[1],
        QueueMode::Replace,
        Order::Move {
            to: pos(5.0, 0.0, 5.0),
        },
    )];

    let mut full = resumable_world();
    let d1 = full.step(&[]).expect("tick 0 steps");
    let d2 = full.step(&second).expect("tick 1 steps");
    assert_eq!(full.head(), d2);

    // The state after the first step, rebuilt from its public fields alone.
    let rebuild = |first: &State| {
        let mut s = State::new(first.seed);
        s.tick = first.tick;
        s.rng = first.rng.clone();
        s.next_id = first.next_id;
        s.armies = first.armies.clone();
        s.entities = first.entities.clone();
        s.counters = first.counters.clone();
        s
    };
    let mut after_first = resumable_world();
    assert_eq!(after_first.step(&[]).expect("tick 0 steps"), d1);

    let mut resumed = rebuild(&after_first);
    resumed.set_head(d1);
    assert_eq!(resumed.head(), d1);
    assert_eq!(resumed.step(&second).expect("tick 1 steps"), d2);

    let mut headless = rebuild(&after_first);
    assert_ne!(headless.step(&second).expect("tick 1 steps"), d2);
}
