use sha2::{Digest as _, Sha256};

use crate::economy::Ledger;
use crate::ids::{Q, Tick};
use crate::state::{ArmyState, Counters, Entity, State};

/// A SHA-256 digest.
pub type Digest = [u8; 32];

/// The chain value before tick 0: thirty-two zero bytes.
pub const GENESIS: Digest = [0; 32];

/// The bit pattern of `x` with every NaN mapped to the canonical quiet NaN
/// `0x7ff8_0000_0000_0000` (DET-022); every other value keeps its bits.
#[must_use]
pub fn canonical_f64_bits(x: f64) -> u64 {
    if x.is_nan() {
        CANONICAL_NAN_BITS
    } else {
        x.to_bits()
    }
}

const CANONICAL_NAN_BITS: u64 = 0x7ff8_0000_0000_0000;

fn put_q(out: &mut Vec<u8>, q: Q) {
    out.extend_from_slice(&q.to_bits().to_le_bytes());
}

/// A collection length as the LE32 the format specifies; a collection over
/// four billion entries cannot occur, and saturating keeps the encoder total.
fn put_len(out: &mut Vec<u8>, len: usize) {
    let len = u32::try_from(len).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
}

/// Append the canonical serialization S of `state` to `out` (DET-009).
///
/// Field order: tick (LE64), seed (LE64), the generator state (LE64), next
/// entity id (LE32), commands applied (LE64), commands ignored (LE64), builds
/// completed (LE64), stall ticks (LE64); the armies, a LE32 count then each in
/// ascending slot order as slot (u8), mass, energy, mass storage, energy
/// storage (each the raw `i64` bits of its [`crate::Q`], LE64), defeated (u8,
/// 0 or 1), then the ledger's eight values (income, requested, and granted for
/// mass then energy, then the two fractions; raw `i64` bits, LE64); the
/// entities, a LE32 count then each in ascending id order as id (LE32), army
/// (u8), blueprint (LE32 length, bytes), x, y, z (each [`canonical_f64_bits`],
/// LE64), health and max health (raw `i64` bits, LE64), built (raw bits),
/// complete (u8), fraction (raw bits), work (u8 0, or u8 1 then the id LE32),
/// then the queued orders (LE32 count, each in
/// [`crate::Order::encode_into`]'s encoding, front to back). The chain head is
/// not part of S.
///
/// A step serializes after advancing the tick, so the bytes of the tick
/// that just ran (`t`) carry the tick field `t+1` (the state after the
/// advance), while the chain link over them is labelled `t`.
pub fn serialize(state: &State, out: &mut Vec<u8>) {
    // Entity, ArmyState, Counters, and Ledger are destructured with no `..`,
    // so a field added to one of them fails to compile here until it is
    // encoded or skipped by name.
    out.extend_from_slice(&state.tick.0.to_le_bytes());
    out.extend_from_slice(&state.seed.to_le_bytes());
    out.extend_from_slice(&state.rng.state().to_le_bytes());
    out.extend_from_slice(&state.next_id.0.to_le_bytes());
    let Counters {
        commands_applied,
        commands_ignored,
        builds_completed,
        stall_ticks,
    } = &state.counters;
    for n in [
        commands_applied,
        commands_ignored,
        builds_completed,
        stall_ticks,
    ] {
        out.extend_from_slice(&n.to_le_bytes());
    }

    put_len(out, state.armies.len());
    for (slot, army) in &state.armies {
        // The slot key is written in place of the army's own id.
        let ArmyState {
            id: _,
            mass,
            energy,
            mass_storage,
            energy_storage,
            defeated,
            ledger,
        } = army;
        out.push(slot.0);
        for q in [mass, energy, mass_storage, energy_storage] {
            put_q(out, *q);
        }
        out.push(u8::from(*defeated));
        let Ledger {
            income_mass,
            income_energy,
            requested_mass,
            requested_energy,
            granted_mass,
            granted_energy,
            fraction_mass,
            fraction_energy,
        } = ledger;
        for q in [
            income_mass,
            income_energy,
            requested_mass,
            requested_energy,
            granted_mass,
            granted_energy,
            fraction_mass,
            fraction_energy,
        ] {
            put_q(out, *q);
        }
    }

    put_len(out, state.entities.len());
    for (key, entity) in &state.entities {
        // The map key is written in place of the entity's own id.
        let Entity {
            id: _,
            army,
            blueprint,
            pos,
            health,
            max_health,
            built,
            complete,
            fraction,
            work,
            orders,
        } = entity;
        out.extend_from_slice(&key.0.to_le_bytes());
        out.push(army.0);
        put_len(out, blueprint.len());
        out.extend_from_slice(blueprint.as_bytes());
        for v in [pos.x, pos.y, pos.z] {
            out.extend_from_slice(&canonical_f64_bits(v).to_le_bytes());
        }
        put_q(out, *health);
        put_q(out, *max_health);
        put_q(out, *built);
        out.push(u8::from(*complete));
        put_q(out, *fraction);
        match work {
            None => out.push(0),
            Some(target) => {
                out.push(1);
                out.extend_from_slice(&target.0.to_le_bytes());
            }
        }
        put_len(out, orders.len());
        for order in orders {
            order.encode_into(out);
        }
    }
}

/// One link of the chain: `SHA-256(prev || LE64(tick) || LE64(len) || state)`
/// (DET-010).
#[must_use]
pub fn chain(prev: &Digest, tick: Tick, state_bytes: &[u8]) -> Digest {
    let mut h = Sha256::new();
    h.update(prev);
    h.update(tick.0.to_le_bytes());
    h.update(
        u64::try_from(state_bytes.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    h.update(state_bytes);
    h.finalize().into()
}

/// The earliest index at which two digest sequences differ, or `None` if they
/// are equal; a proper prefix differs at its own length (DET-011).
#[must_use]
pub fn first_divergence(a: &[Digest], b: &[Digest]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .or_else(|| (a.len() != b.len()).then_some(a.len().min(b.len())))
}
