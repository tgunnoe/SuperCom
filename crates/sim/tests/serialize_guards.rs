//! The canonical serialization, byte for byte, over a state in which every
//! field holds a distinct non-default value (docs/specs/determinism.md,
//! DET-009). The expected vector is assembled by hand from the documented
//! order out of literal integers; it never calls the serializer.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::VecDeque;

use supercom_sim::{
    Army, ArmyState, Entity, EntityId, Layer, Ledger, Outcome, Pos, Projectile, ProjectileId, Q,
    State, Tick, WeaponState, serialize,
};

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
#[allow(
    clippy::too_many_lines,
    reason = "one vector pins every byte of the state, so it reads top to bottom"
)]
fn serialization_pins_every_field_in_the_documented_order() {
    let mut s = State::new(0x0102_0304_0506_0708);
    s.tick = Tick(0x11);
    s.next_id = EntityId(0x2233);
    s.counters.commands_applied = 0x31;
    s.counters.commands_ignored = 0x32;
    s.counters.builds_completed = 0x33;
    s.counters.stall_ticks = 0x34;
    s.counters.moves_completed = 0x35;
    s.counters.projectile_impacts = 0x36;
    s.counters.deaths = 0x37;
    s.counters.defeats = 0x38;
    s.counters.victories = 0x39;
    s.outcome = Some(Outcome::Winner(Army(7)));
    s.next_projectile_id = ProjectileId(0x4455);
    s.projectiles.insert(
        ProjectileId(0x66),
        Projectile {
            id: ProjectileId(0x66),
            army: Army(3),
            source: EntityId(0x77),
            kind: Some("shell".to_owned()),
            pos: Pos {
                x: 1.5,
                y: 2.5,
                z: 3.5,
            },
            vel: [0.5, -0.25, 4.0],
            target: Some(EntityId(0x88)),
            aim: Pos {
                x: 6.5,
                y: 7.5,
                z: 8.5,
            },
            damage: q(0x99),
            damage_radius: 2.0,
            damage_type: "Normal".to_owned(),
            lifetime: Some(1.5),
        },
    );
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
            heading: 0.5,
            speed: 2.0,
            layer: Layer::Water,
            route: vec![(3, 4), (5, 6)],
            goal: Some(Pos {
                x: 4.5,
                y: 5.5,
                z: 6.5,
            }),
            maintenance_fraction: q(0x65),
            weapons: vec![
                WeaponState {
                    target: Some(EntityId(0x99)),
                    cooldown: 0x1122,
                },
                WeaponState {
                    target: None,
                    cooldown: 0x33,
                },
            ],
            orders: VecDeque::new(),
        },
    );

    let mut want = Vec::new();
    // Tick, seed, generator state (a fresh generator holds the seed), next
    // entity id, then the nine counters.
    le64(&mut want, 0x11);
    le64(&mut want, 0x0102_0304_0506_0708);
    le64(&mut want, 0x0102_0304_0506_0708);
    le32(&mut want, 0x2233);
    for c in [0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39] {
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
    // its id, heading and speed as f64 bits, the layer's declaration index
    // (Water is 5), the route as a count and two LE32 per cell, the goal
    // present and its x, y, z as f64 bits, the maintenance fraction, the
    // weapons, and the queue.
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
    le64(&mut want, 0x3FE0_0000_0000_0000);
    le64(&mut want, 0x4000_0000_0000_0000);
    want.push(5);
    le32(&mut want, 2);
    for cell in [3, 4, 5, 6] {
        le32(&mut want, cell);
    }
    want.push(1);
    for bits in [
        0x4012_0000_0000_0000,
        0x4016_0000_0000_0000,
        0x401A_0000_0000_0000,
    ] {
        le64(&mut want, bits);
    }
    raw(&mut want, 0x65);
    // Two weapons: the first holds a target, the second none.
    le32(&mut want, 2);
    want.push(1);
    le32(&mut want, 0x99);
    le32(&mut want, 0x1122);
    want.push(0);
    le32(&mut want, 0x33);
    // An empty queue, then the next projectile id and one projectile: id,
    // army, source, kind present and its length-prefixed string, position,
    // velocity, target present and its id, aim, damage, damage radius,
    // damage type, and lifetime present and its f64 bits.
    le32(&mut want, 0);
    le32(&mut want, 0x4455);
    le32(&mut want, 1);
    le32(&mut want, 0x66);
    want.push(3);
    le32(&mut want, 0x77);
    want.push(1);
    le32(&mut want, 5);
    want.extend_from_slice(b"shell");
    for bits in [
        0x3FF8_0000_0000_0000,
        0x4004_0000_0000_0000,
        0x400C_0000_0000_0000,
        0x3FE0_0000_0000_0000,
        0xBFD0_0000_0000_0000,
        0x4010_0000_0000_0000,
    ] {
        le64(&mut want, bits);
    }
    want.push(1);
    le32(&mut want, 0x88);
    for bits in [
        0x401A_0000_0000_0000,
        0x401E_0000_0000_0000,
        0x4021_0000_0000_0000,
    ] {
        le64(&mut want, bits);
    }
    raw(&mut want, 0x99);
    le64(&mut want, 0x4000_0000_0000_0000);
    le32(&mut want, 6);
    want.extend_from_slice(b"Normal");
    want.push(1);
    le64(&mut want, 0x3FF8_0000_0000_0000);
    // The outcome: present, the winner tag, and the army.
    want.extend_from_slice(&[1, 0, 7]);

    let mut got = Vec::new();
    serialize(&s, &mut got);
    assert_eq!(got, want);
}

/// A draw ends the stream with the empty projectile count, then the
/// present flag and the draw tag.
#[test]
fn draw_outcome_encodes_as_present_then_the_draw_tag() {
    let mut s = State::new(1);
    s.outcome = Some(Outcome::Draw);
    let mut got = Vec::new();
    serialize(&s, &mut got);
    let mut tail = Vec::new();
    le32(&mut tail, 0);
    tail.extend_from_slice(&[1, 1]);
    assert!(
        got.ends_with(&tail),
        "the tail is {:?}",
        &got[got.len().saturating_sub(tail.len())..]
    );
}
