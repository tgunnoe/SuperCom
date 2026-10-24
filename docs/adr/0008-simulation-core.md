# ADR-0008: The simulation core's design choices

**Status:** ACCEPTED (ratified by the head on 2026-10-18, in one ruling over the six simulation records)

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
  Intel, Victory. Each adjacency has a reason. Commands run first so a
  command's effect is in place before any other update of its tick
  (DET-003). Economy precedes Build because building consumes what the
  economy credited. Build precedes Movement so a unit spawned by Build
  moves from the next tick, not this one. Movement precedes Weapons so
  targets are acquired at settled positions. Weapons, then Projectiles,
  then Damage: a shot is fired, flies, then lands. Damage precedes Death so
  health reaches zero before removal and removals take effect after all
  damage of the tick. Death precedes Intel and Victory because the dead
  neither see nor count; Victory runs last on the settled state. A phase
  visits the entities present when it starts, in ascending id (SIM-036), so
  an entity created during a phase is first visited by the next phase. The
  original's order is unobserved; a later observation that contradicts this
  order is a new decision, not a silent edit. Checked by
  `sim_tick_phases_in_order`, which pins the order, not the reasons.
- **Number representation (D3).** Resources, health, progress, and timers are
  `I32F32` fixed point; geometry is `f64` under ADR-0006's exact operations
  (`+ - * /`, `sqrt`, `mul_add`, `libm` transcendentals), canonicalized at the
  serialization boundary. No `f32` appears in any state type.
- **Canonical state serialization (DET-009).** One fixed field order, little
  endian integers, fixed-point values as raw bits, `f64` values as canonical
  bit patterns, every map in key order. Bytes are what the hash chain of
  DET-010 consumes. The bytes serialized at the end of tick `t` carry the
  tick field `t+1` (the state after the advance) while the chain link is
  labelled `t`; the chain head lies outside S. Checked by
  `sim_serialization_is_canonical`, `sim_tick_hash_chain_bits`, and
  `sim_step_digest_is_chain_over_the_end_of_tick_state`.
- **Command record and application (SIM-045).** A command carries tick,
  source slot, sequence number, army, unit ids, one order, and a replace or
  append mode. Commands stamped for a tick apply at its start in the order
  the caller passes, which is schedule order on the replay path (the order
  within a batch is the caller's contract, because DET-013's control needs
  to vary it). `step` refuses, before changing anything, a command not
  stamped for the tick being run (`Error::WrongTick`) and a `(source, seq)`
  pair repeated within the batch (`Error::Schedule`). Application is
  all-or-nothing: a command whose unit list is empty, or names a unit that
  does not exist or belongs to another army, is ignored whole and counted,
  and the counters are part of the state. A unit that died between issue
  and application is the missing-unit case: the command naming it is voided
  whole and counted (SIM-045), and what that costs a group order is a debt
  for the HUD and AI families. Checked by
  `sim_command_application_and_ownership`,
  `sim_mixed_ownership_command_is_ignored_whole`, and the two
  `step_refuses_` tests.
- **Snapshot (SIM-046).** Readers see an owned snapshot, entities and armies
  ascending by id, never a borrow of the live state. Checked by
  `sim_snapshot_lists_entities_in_id_order`.
- **Generator (DET-025).** The sim holds its own pseudo-random generator,
  seeded from the initial state alone: splitmix64, whose state is the seed,
  advanced by the increment `0x9E3779B97F4A7C15` and then mixed with two
  xor-shift-multiply rounds (shifts 30 and 27, multipliers
  `0xBF58476D1CE4E5B9` and `0x94D049BB133111EB`) and a final shift of 31.
  Its 64-bit state is written in S as one little-endian word. The reason:
  S carries one word of generator state, which a 256-bit generator does not
  fit without widening the seam, and no external crate is used, so the
  output cannot change with a dependency update. Checked by
  `sim_rng_is_seeded_from_initial_state`,
  `sim_rng_matches_the_reference_splitmix64`,
  `sim_rng_golden_vector_for_seed_1234567`, and
  `sim_serialized_generator_field_is_the_state_after_a_draw`.
- **Replay header and layout (DET-007, DET-008, DET-026).** A replay opens
  with the magic `SCREPLAY` and a format version (1), then the engine
  version, the map name, the map's content digest and the installed-data
  catalog digest (the installed game data is identified by those two
  SHA-256 digests, which `check_content` compares before tick 0), the
  plugin list, the player slots, and the 64-bit seed. The plugin list is
  empty in v0 because the first release runs no plugins (Q4), but is a list
  of name and digest pairs now so a later plugin set does not change the
  layout; the reader parses any length and `check_content` refuses a
  non-empty list, as this version installs no plugin. Player i is the slot of source i, and
  a record must name that slot's army, which binds a command's army to the
  source that issued it. The tick rate is not stored: SIM-001 fixes it, so
  a stored copy could only disagree. The command log follows in schedule
  order and the checkpoints after it as ascending (tick, digest) pairs.
  The reader refuses a repeated `(source, seq)` anywhere in the log (the
  schedule's per-call check cannot see a pair repeated across ticks) and
  checks every length and count against the remaining input before it
  allocates, so it never allocates a buffer larger than the input; the
  decoded value is linear in the input with a constant factor of about
  four for a log of minimal commands (measured at about 4.4 times for a
  million of them), not bounded by the input's size. The reader is
  strict: it accepts exactly the bytes the writer can produce, so it
  refuses bytes after the last checkpoint and any non-canonical NaN
  (DET-022), and a length error names the first byte of the length
  field. A resumed run needs the chain head, which
  lies outside S, so `State` gains `head` and `set_head`. Checked by
  `sim_replay_round_trip`, `sim_replay_rejects_malformed`,
  `sim_replay_fixture_is_byte_exact`, `sim_run_resumes_from_a_checkpoint_head`,
  and the guard tests in `crates/sim/tests/replay_guards.rs`.

## Consequences

### Positive

- The phase order, state shape, and generator are fixed before rules are
  written, so every rule has one place to run and every test one state to
  build.
- Each decision bullet names the tests that check it, so a deviation shows
  as a red test; the phase reasons and the visiting order are not tested
  until rules exist to depend on them.

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
- Clauses DET-007, DET-008, DET-009, DET-025, DET-026, SIM-044, SIM-045,
  SIM-046, and SIM-036, SIM-018.
