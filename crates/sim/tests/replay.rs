//! Acceptance tests of the replay format (docs/specs/determinism.md, DET-007,
//! DET-008, DET-026). All synthetic: the fixture is built here by a reference
//! encoder that shares no code with the crate's writer.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_sim::{
    Army, Command, Digest, EntityId, Order, Pos, QueueMode, Replay, ReplayHeader, Tick,
};

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
            format_version: 1,
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
