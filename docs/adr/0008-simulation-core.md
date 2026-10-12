# ADR-0008: The simulation core's design choices

**Status:** PROPOSED

**Date:** 2026-10-12

## Context

`docs/specs/sim.md` and `docs/specs/determinism.md` specify what the original
engine does where the installed game and the reference stubs show it, and
leave the structure of the new sim open where the original exposes nothing:
the order of the phases within a tick, the shape of a command and of the
state, the number representation of each quantity, and the pseudo-random
generator. ADR-0003 chose a deterministic Rust sim driven by a command stream
and ADR-0006 chose the float vocabulary. The crate `crates/sim` must commit to
the remaining choices before any rule is written, and the original's own tick
order is unobserved: nothing in the installed game or the stubs fixes it, so
every choice below is this project's design and says so (AGENTS.md R2, R7).
This record lists them by clause so the specification's design clauses have a
ratifiable source.

## Decision

- **Tick phases (SIM-044).** One tick runs ten phases in a fixed order:
  Commands, Economy, Build, Movement, Weapons, Projectiles, Damage, Death,
  Intel, Victory. Commands run first so a command's effect is in place before
  any other update of its tick (DET-003); Death follows Damage so removals
  take effect after all damage of the tick; Victory runs last on the settled
  state. The original's order is unobserved; a later observation that
  contradicts this order is a new decision, not a silent edit.
- **Number representation (D3).** Resources, health, progress, and timers are
  `I32F32` fixed point; geometry is `f64` under ADR-0006's exact operations
  (`+ - * /`, `sqrt`, `mul_add`, `libm` transcendentals), canonicalized at the
  serialization boundary. No `f32` appears in any state type.
- **Canonical state serialization (DET-009).** One fixed field order, little
  endian integers, fixed-point values as raw bits, `f64` values as canonical
  bit patterns, every map in key order. Bytes are what the hash chain of
  DET-010 consumes.
- **Command record and application (SIM-045).** A command carries tick,
  source slot, sequence number, army, unit ids, one order, and a replace or
  append mode. Commands stamped for a tick apply at its start in schedule
  order; a command addressing a unit its army does not own is ignored and
  counted, and the counters are part of the state.
- **Snapshot (SIM-046).** Readers see an owned snapshot, entities and armies
  ascending by id, never a borrow of the live state.
- **Generator (DET-025).** The sim holds its own pseudo-random generator,
  written in the crate, seeded from the initial state alone; no external
  randomness crate is used, so its output cannot change with a dependency
  update.

## Consequences

### Positive

- The phase order, state shape, and generator are fixed before rules are
  written, so every rule has one place to run and every test one state to
  build.
- Each choice is checked by a named test, so a deviation shows as a red test.

### Negative

- If the original's tick order is later observed and differs, behaviour that
  depends on it (for example a unit that dies and fires in the same tick)
  differs from the original until the order is amended here.
- A generator of this project's own is one more component to verify.

## Alternatives Considered

- `f64` for every quantity: rejected; accumulating resources and timers in
  floats makes rounding order part of the contract.
- A third-party generator crate: rejected; its output stability across
  versions is not under the project's control.

## References

- ADR-0003 (the sim stack) and ADR-0006 (the float strategy).
- Clauses DET-009, DET-025, SIM-044, SIM-045, SIM-046, and SIM-036, SIM-018.
