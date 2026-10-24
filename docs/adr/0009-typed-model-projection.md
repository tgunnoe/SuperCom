# ADR-0009: The typed model's projection of capability tables and ManualFire

**Status:** ACCEPTED (ratified by the head on 2026-10-18, in one ruling over the six simulation records)

**Date:** 2026-10-12

## Context

ADR-0002 makes Nickel contracts the validator of the blueprint data and the
typed Rust model a projection of the parsed value tree. The projection fixes
a representation wherever the installed data stores one fact in several
shapes. Two such choices are not forced by the data and are recorded here.

A capability table such as `General.CommandCaps` stores a Boolean per key,
in whatever order the blueprint wrote it, and the simulation needs a stable
order. A weapon's `ManualFire` is stored as a Boolean on 13 installed
entries and as the number 1 on 9 (DAT-035).

## Decision

DAT-037: `General.CommandCaps` projects to the keys whose value is true,
in ascending order, whatever the order the table stores. Sorting makes the
projection a function of the table's contents rather than of the file's
layout, so a reordered blueprint cannot change simulation behaviour, and the
sorted list compares and hashes without a second normalisation step. The
same shape is used for `Physics.BuildOnLayerCaps` (DAT-039).

DAT-038: `ManualFire` stored as a Boolean reads as itself, stored as the
number 1 reads as true, and any other value is a type error from both the
contract and the projection. The contract states the same predicate the
projection implements, so the two verdicts cannot differ (DAT-011), and
refusing every other value keeps a mistyped blueprint from silently reading
as a weapon that is or is not manually fired.

## Consequences

### Positive

- The projection of a capability table is independent of stored order.
- The number 1 of the installed data is accepted without widening the
  contract to arbitrary values.

### Negative

- A mod storing `ManualFire` as another number, such as 2 or `true` written
  as a string, is refused where the original engine's Lua truthiness might
  accept it; relaxing the rule is a new decision.
- Sorting discards the stored order of the table, which the data shows
  carries no meaning but which a future clause could want.
