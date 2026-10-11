# ADR-0002: Canonical game data is Nickel with contracts

**Status:** ACCEPTED

**Date:** 2026-10-07

---

## Context

Unit, projectile, prop, and effect definitions in the installed game are
`.bp` files: table constructors in the GPG Lua dialect (`UnitBlueprint {
... }`, `Sound { Bank = ..., Cue = ... }`, `<LOC ...>` strings, `#` comments).
`units.scd` alone holds 580 of them, with a few hundred more across
`projectiles.scd`, `props.scd`, `effects.scd`, and `env.scd`. Nothing
validates them: a mistyped field is a missing value at runtime and the unit
quietly misbehaves. Mods override blueprints with Lua hook files that patch
tables at load time.

The project needs a data model that loads from the installed game without
copying anything into the repository, validates every record, supports
layered overrides for mods, and is pure Rust.

## Decision

- Canonical data lives in Nickel: typed records with contracts for every
  blueprint family (unit, weapon, projectile, prop, effect) in `crates/data`.
- `crates/bp-import` parses the installed `.bp` files with a small parser for
  the data subset of the GPG Lua dialect, converts them into the Nickel
  model, and caches the evaluated result in the user data directory. The
  cache is never committed.
- Mods are Nickel files merged over the base records with priorities; the
  contract check runs after the merge, so an overlay cannot produce an invalid
  record.
- Evaluation uses `nickel-lang-core` in-process. The serialized cache keeps
  evaluation off the hot path.

## Consequences

### Positive

- Load-time validation with field-level error messages.
- Mod overlays come from Nickel's record merging; no custom patch format.
- One configuration language across the project and predicate.

### Negative

- Evaluation cost of the full blueprint set is unmeasured (AGENTS.md U2).
- Import resolution through the library, rather than the CLI, needs
  confirming.
- Modders learn Nickel instead of editing Lua tables.
- The v0 importer validates presence and types in Rust before Nickel and
  caches the Rust projection, so the post-merge contract guarantee does not
  yet hold for overlays; docs/specs/data-model.md states that pipeline as an
  interim toward this decision (the head's ruling of 2026-10-10).

---

## Alternatives Considered

| Alternative               | Reason Rejected                                                                                                                 |
| :------------------------ | :------------------------------------------------------------------------------------------------------------------------------ |
| RON + serde               | Zero-friction into Rust structs, but no contracts beyond the type and no merge semantics; overlays need a bespoke patch format. |
| KDL                       | Readable, but no logic or merging; the same overlay problem.                                                                    |
| Keep `.bp` plus Lua hooks | Requires the Lua dialect at runtime (see ADR-0001) and keeps the unvalidated model.                                             |
| JSON or TOML              | No contracts or merging; verbose for deeply nested blueprint data.                                                              |

---

## References

- AGENTS.md: R4, U2.
- Nickel: https://nickel-lang.org and https://docs.rs/nickel-lang-core/
- Related ADRs: ADR-0001 (plugin boundary).
