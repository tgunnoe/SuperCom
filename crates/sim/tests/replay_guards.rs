//! Guard tests of the replay reader and writer (DET-007, DET-008, DET-022,
//! DET-026): each pins a refusal or an encoding that a mutation of the
//! reader, the writer, or the command codec would otherwise leave green.
//! All synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_sim::{
    Army, Command, Digest, EntityId, Error, Order, Pos, QueueMode, REPLAY_FORMAT_VERSION, Replay,
    ReplayError, ReplayHeader, Tick, check_content, read_replay, write_replay,
};

/// The committed golden vector of DET-026, read at compile time.
const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/synthetic.replay"
));

const CANONICAL_NAN: u64 = 0x7ff8_0000_0000_0000;
const ODD_NAN: u64 = 0x7ff0_0000_0000_0001;
const ENGINE: &str = "eng";
const MAP: &str = "map";

fn digest(b: u8) -> Digest {
    [b; 32]
}

fn header(plugins: Vec<(String, Digest)>) -> ReplayHeader {
    ReplayHeader {
        format_version: REPLAY_FORMAT_VERSION,
        engine_version: ENGINE.to_owned(),
        map: MAP.to_owned(),
        map_digest: digest(1),
        catalog_digest: digest(2),
        plugins,
        players: vec![(Army(0), "a".to_owned()), (Army(1), "b".to_owned())],
        seed: 7,
    }
}

fn command(seq: u32, units: Vec<EntityId>, order: Order) -> Command {
    Command {
        tick: Tick(1),
        source: 0,
        seq,
        army: Army(0),
        units,
        order,
        mode: QueueMode::Replace,
    }
}

fn replay(plugins: Vec<(String, Digest)>, commands: Vec<Command>) -> Replay {
    Replay {
        header: header(plugins),
        commands,
        checkpoints: Vec::new(),
    }
}

fn written(r: &Replay) -> Vec<u8> {
    let mut out = Vec::new();
    write_replay(r, &mut out);
    out
}

fn encoded(c: &Command) -> Vec<u8> {
    let mut out = Vec::new();
    c.encode_into(&mut out);
    out
}

/// Byte offset of the plugin count: magic, version, two strings, two digests.
fn plugin_count_at() -> usize {
    8 + 4 + (4 + ENGINE.len()) + (4 + MAP.len()) + 64
}

fn offset_of(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
        .expect("the needle occurs in the haystack")
}

fn replay_error(r: Result<impl Sized, Error>) -> ReplayError {
    match r {
        Err(Error::Replay(e)) => e,
        Err(other) => panic!("expected a replay error, got {other}"),
        Ok(_) => panic!("expected a refusal, got a value"),
    }
}

fn move_to(x: f64) -> Order {
    Order::Move {
        to: Pos { x, y: 1.0, z: 2.0 },
    }
}

// ---- trailing bytes ----------------------------------------------------------

#[test]
fn trailing_bytes_after_the_last_checkpoint_are_refused() {
    assert!(read_replay(FIXTURE).is_ok());
    let mut longer = FIXTURE.to_vec();
    longer.extend_from_slice(&[0, 1, 2]);
    assert_eq!(
        replay_error(read_replay(&longer)),
        ReplayError::TrailingBytes { at: FIXTURE.len() }
    );
}

// ---- NaN bits ---------------------------------------------------------------

#[test]
fn reader_refuses_a_non_canonical_nan_with_its_offset() {
    let cmd = command(0, vec![], move_to(1.0));
    let r = replay(vec![], vec![cmd.clone()]);
    let mut bytes = written(&r);
    let enc = encoded(&cmd);
    // tick, source, seq, army, unit count, mode, tag precede x.
    let x_at = offset_of(&bytes, &enc) + 8 + 1 + 4 + 1 + 4 + 1 + 1;
    for bits in [ODD_NAN, ODD_NAN | (1 << 63), u64::MAX] {
        bytes[x_at..x_at + 8].copy_from_slice(&bits.to_le_bytes());
        assert_eq!(
            replay_error(read_replay(&bytes)),
            ReplayError::NonCanonicalFloat { at: x_at },
            "bits {bits:#x}"
        );
    }
    bytes[x_at..x_at + 8].copy_from_slice(&CANONICAL_NAN.to_le_bytes());
    assert!(read_replay(&bytes).is_ok(), "the canonical NaN is accepted");
}

#[test]
fn order_encoder_writes_canonical_nan_bits() {
    for bits in [ODD_NAN, ODD_NAN | (1 << 63), u64::MAX] {
        let mut out = Vec::new();
        move_to(f64::from_bits(bits)).encode_into(&mut out);
        assert_eq!(out[0], 0);
        assert_eq!(out[1..9], CANONICAL_NAN.to_le_bytes(), "bits {bits:#x}");

        // Compared by bytes: a NaN is never equal to itself.
        let cmd = command(0, vec![], move_to(f64::from_bits(bits)));
        let enc = encoded(&cmd);
        let (back, used) = Command::decode(&enc).expect("canonical bytes decode");
        assert_eq!(used, enc.len());
        assert_eq!(encoded(&back), enc);
    }
}

// ---- the plugin list ----------------------------------------------------------

#[test]
fn a_plugin_entry_is_name_then_digest_and_check_content_refuses_it() {
    let name = "plug";
    let d = digest(0x55);
    let r = replay(vec![(name.to_owned(), d)], vec![]);
    let bytes = written(&r);

    let mut entry = u32::try_from(name.len())
        .expect("small")
        .to_le_bytes()
        .to_vec();
    entry.extend_from_slice(name.as_bytes());
    entry.extend_from_slice(&d);
    let at = plugin_count_at();
    assert_eq!(bytes[at..at + 4], 1u32.to_le_bytes());
    assert_eq!(bytes[at + 4..at + 4 + entry.len()], entry[..]);

    let back = read_replay(&bytes).expect("a plugin list parses");
    assert_eq!(back, r);
    assert_eq!(
        replay_error(check_content(&back.header, &digest(1), &digest(2))),
        ReplayError::PluginsUnsupported { count: 1 }
    );
    let none = replay(vec![], vec![]);
    assert!(check_content(&none.header, &digest(1), &digest(2)).is_ok());
}

// ---- player slots -------------------------------------------------------------

#[test]
fn a_record_from_a_source_with_no_slot_is_refused() {
    let mut cmd = command(0, vec![], Order::Stop);
    cmd.source = 9;
    let r = replay(vec![], vec![cmd]);
    assert_eq!(
        replay_error(read_replay(&written(&r))),
        ReplayError::ArmyNotInSlot { index: 0 }
    );
}

// ---- hostile counts -----------------------------------------------------------

#[test]
fn a_unit_count_of_u32_max_is_refused_at_its_first_byte() {
    let mut enc = encoded(&command(0, vec![], Order::Stop));
    enc[14..18].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        replay_error(Command::decode(&enc)),
        ReplayError::LengthExceedsInput { at: 14 }
    );
}

#[test]
fn plugin_and_player_counts_of_u32_max_are_refused_at_their_first_byte() {
    let plugins_at = plugin_count_at();
    let mut bytes = written(&replay(vec![], vec![]));
    bytes[plugins_at..plugins_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        replay_error(read_replay(&bytes)),
        ReplayError::LengthExceedsInput { at: plugins_at }
    );

    let players_at = plugins_at + 4;
    let mut bytes = written(&replay(vec![], vec![]));
    bytes[players_at..players_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        replay_error(read_replay(&bytes)),
        ReplayError::LengthExceedsInput { at: players_at }
    );
}

#[test]
fn a_string_length_error_names_the_length_field() {
    // Magic and version, then the engine string's length at byte 12.
    let mut bytes = b"SCREPLAY".to_vec();
    bytes.extend_from_slice(&REPLAY_FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        replay_error(read_replay(&bytes)),
        ReplayError::LengthExceedsInput { at: 12 }
    );
}

// ---- the minimum command size --------------------------------------------------

#[test]
fn the_smallest_command_is_twenty_bytes_and_a_log_of_them_parses() {
    let stop = command(0, vec![], Order::Stop);
    assert_eq!(encoded(&stop).len(), 20);

    // The reader bounds a count by its minimum encoding against the input
    // left; a bound above the true size refuses a log it should accept.
    for n in 1..=8u32 {
        let commands: Vec<Command> = (0..n)
            .map(|seq| command(seq, vec![], Order::Stop))
            .collect();
        let r = replay(vec![], commands);
        assert_eq!(read_replay(&written(&r)).expect("a minimal log parses"), r);
    }
}

// ---- Command::decode ------------------------------------------------------------

fn tag(order: &Order) -> u8 {
    // Exhaustive: a new Order variant fails to compile here until its
    // decoder is covered by `every_order_round_trips_through_decode`.
    match order {
        Order::Move { .. } => 0,
        Order::Stop => 1,
        Order::Attack { .. } => 2,
        Order::BuildMobile { .. } => 3,
        Order::BuildFactory { .. } => 4,
        Order::Guard { .. } => 5,
    }
}

#[test]
fn every_order_round_trips_through_decode() {
    let orders = [
        move_to(-3.5),
        Order::Stop,
        Order::Attack {
            target: EntityId(77),
        },
        Order::BuildMobile {
            blueprint: "uel0001".to_owned(),
            at: Pos {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
        },
        Order::BuildFactory {
            blueprint: "ueb0101".to_owned(),
        },
        Order::Guard {
            target: EntityId(9),
        },
    ];
    let mut tags: Vec<u8> = orders.iter().map(tag).collect();
    tags.sort_unstable();
    assert_eq!(tags, [0, 1, 2, 3, 4, 5]);

    for (i, order) in orders.into_iter().enumerate() {
        for mode in [QueueMode::Replace, QueueMode::Append] {
            let mut cmd = command(
                u32::try_from(i).expect("small"),
                vec![EntityId(1), EntityId(2)],
                order.clone(),
            );
            cmd.mode = mode;
            let mut enc = encoded(&cmd);
            assert_eq!(enc[8 + 1 + 4 + 1 + 4 + 8 + 1], tag(&order));
            let len = enc.len();
            enc.extend_from_slice(&[0xee; 5]);
            let (back, used) = Command::decode(&enc).expect("an encoded command decodes");
            assert_eq!(back, cmd);
            assert_eq!(used, len, "bytes consumed exclude what follows");
        }
    }
}
