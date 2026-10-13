//! The canonical serialization, byte for byte, over a state in which every
//! field holds a distinct non-default value (docs/specs/determinism.md,
//! DET-009). The expected vector is assembled by hand from the documented
//! order out of literal integers; it never calls the serializer.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::VecDeque;

use supercom_sim::{Army, ArmyState, Entity, EntityId, Ledger, Pos, Q, State, Tick, serialize};

fn le64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn le32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// A fixed-point value as the raw `i64` bits the serializer writes.
fn raw(out: &mut Vec<u8>, bits: i64) {
    out.extend_from_slice(&bits.to_le_bytes());
}

/// The fixed-point value with these raw bits.
fn q(bits: i64) -> Q {
    Q::from_bits(bits)
}

#[test]
fn serialization_pins_every_field_in_the_documented_order() {
    let mut s = State::new(0x0102_0304_0506_0708);
    s.tick = Tick(0x11);
    s.next_id = EntityId(0x2233);
    s.counters.commands_applied = 0x31;
    s.counters.commands_ignored = 0x32;
    s.counters.builds_completed = 0x33;
    s.counters.stall_ticks = 0x34;
    s.armies.insert(
        Army(7),
        ArmyState {
            id: Army(7),
            mass: q(0x41),
            energy: q(0x42),
            mass_storage: q(0x43),
            energy_storage: q(0x44),
            defeated: true,
            ledger: Ledger {
                income_mass: q(0x51),
                income_energy: q(0x52),
                requested_mass: q(0x53),
                requested_energy: q(0x54),
                granted_mass: q(0x55),
                granted_energy: q(0x56),
                fraction_mass: q(0x57),
                fraction_energy: q(0x58),
            },
        },
    );
    s.entities.insert(
        EntityId(5),
        Entity {
            id: EntityId(5),
            army: Army(7),
            blueprint: "abc".to_owned(),
            // 1.5, 2.5, and 3.5 are exact in binary64.
            pos: Pos {
                x: 1.5,
                y: 2.5,
                z: 3.5,
            },
            health: q(0x61),
            max_health: q(0x62),
            built: q(0x63),
            complete: false,
            fraction: q(0x64),
            work: Some(EntityId(0x7788)),
            orders: VecDeque::new(),
        },
    );

    let mut want = Vec::new();
    // Tick, seed, generator state (a fresh generator holds the seed), next
    // entity id, then the four counters.
    le64(&mut want, 0x11);
    le64(&mut want, 0x0102_0304_0506_0708);
    le64(&mut want, 0x0102_0304_0506_0708);
    le32(&mut want, 0x2233);
    for c in [0x31, 0x32, 0x33, 0x34] {
        le64(&mut want, c);
    }
    // One army: slot, four amounts, defeated, the ledger's eight values in
    // income, requested, granted order for mass then energy, then fractions.
    le32(&mut want, 1);
    want.push(7);
    for bits in [0x41, 0x42, 0x43, 0x44] {
        raw(&mut want, bits);
    }
    want.push(1);
    for bits in [0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58] {
        raw(&mut want, bits);
    }
    // One entity: id, army, blueprint length and bytes, x, y, z as f64
    // bits, health, max health, built, complete, fraction, work present and
    // its id, and an empty queue.
    le32(&mut want, 1);
    le32(&mut want, 5);
    want.push(7);
    le32(&mut want, 3);
    want.extend_from_slice(b"abc");
    le64(&mut want, 0x3FF8_0000_0000_0000);
    le64(&mut want, 0x4004_0000_0000_0000);
    le64(&mut want, 0x400C_0000_0000_0000);
    for bits in [0x61, 0x62, 0x63] {
        raw(&mut want, bits);
    }
    want.push(0);
    raw(&mut want, 0x64);
    want.push(1);
    le32(&mut want, 0x7788);
    le32(&mut want, 0);

    let mut got = Vec::new();
    serialize(&s, &mut got);
    assert_eq!(got, want);
}
