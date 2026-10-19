# ADR-0015: The replay harness: one log, two runs, two machines

**Status:** PROPOSED

**Date:** 2026-10-18

## Context

The determinism clauses promise that a command log replayed twice gives the
same digest at every tick (DET-010) and that the claim of equality across
x86_64 and aarch64 waits on one log replayed on both (DET-024). Until now the
evidence was synthetic: reference models in the determinism bench and the
sim's own golden worlds. The installed maps and blueprints were exercised by
single-purpose conformance tests, never by one long match that touches every
rule family. This record lists the choices by clause so the design clauses
have a ratifiable source (AGENTS.md R2, R7).

## Decision

- **The committed log is a generated data file (DET-027).** A skirmish on
  the installed map SCMP_012 is scripted in code, run once, and written by an
  ignored test of the harness crate, which is the only writer. The file holds
  blueprint ids, a map name, two digests, positions, ticks, and checkpoint
  digests: identifiers and numbers, under the clean-room scan's size cap of
  4 KiB, with the install's content digests in its header as numbers. A
  checkpoint written by an earlier process is what a later process checks
  its digests against.
- **The swept comparison and the checkpoints are the one-machine evidence
  (DET-027, DET-028).** Two runs in one process agree at every tick and agree
  with the file's checkpoints; moving one command by one tick moves the first
  divergence to exactly that tick, which shows the comparison can fail.
- **The log covers the manifest (DET-029).** At its end moves have
  completed, builds have completed, projectiles have hit, entities have died,
  the economy has stalled, one army has been defeated, and one victory has
  been declared. A log that exercised only some rules would leave the rest to
  the cross-host comparison unobserved.
- **A player's header string names its faction (DET-030).** Playback needs
  nothing beyond the file and the installed data (DET-007), so the string the
  format already carries is read as `UEF`, `Aeon`, `Cybran`, or `Seraphim`,
  and the slot of the player at index i is i + 1. No field is added and the
  format version stays 1. A format version 2 with a faction field of its own
  waits for the first need for a player display name, which is what the
  string would then carry.
- **The payload-NaN case places two NaNs in an entity's position (DET-024).**
  One NaN is computed by the host's own arithmetic from zero over zero at run
  time, through an optimization barrier so the compiler cannot fold it; the
  other is the result a plugin component returns through the plugin host. An
  extra entity is spawned at a position holding them and ordered to move; ten
  ticks are stepped and each digest printed. The digests agree across hosts
  only because the serialization canonicalizes every NaN (DET-022). The raw
  bit patterns of both NaNs and of the entity's final position are printed on
  standard error, outside the compared output.
- **The recursion case compares the kind of trap, and records the depth
  (DET-024).** DET-023 says the host's stack budget differs between hosts, so
  the trap cannot fall on the same tick on both. A component whose step
  recurses to the depth its argument names is probed by bisection, a fresh
  instance per probe, for the largest depth that returns; that depth is
  printed on standard error and recorded in ADR-0006, never compared. An
  unbounded recursion is stepped once and its trap must be stack exhaustion,
  not fuel exhaustion or a crash; that line is compared. This is a departure
  from DET-024's earlier wording that the trap falls on the same tick on
  both; it is stated here for the head to ratify at the pull request. A
  plugin entering the tick loop would need a deterministic depth bound below
  the smaller measured depth. This version runs no plugin inside the tick
  loop: the harness drives the host from outside the sim.
- **Two processes must agree (DET-027).** The digest program of the harness
  crate prints one line per tick and the two cases; its test runs it twice as
  child processes and requires equal standard output. Its standard output is
  host-independent by construction.
- **The cross-host run is a script (DET-024).** `tools/replay_compare.sh`
  runs the digest program on this host and, over ssh, on another checkout of
  the same commit, and compares the outputs byte for byte. `--local` runs it
  twice here. The cross-host run is made by hand at a green tip; ADR-0006's
  aarch64 section records its output: both hosts' architecture lines, the
  commit, the equality with the line count, each host's raw NaN bits and
  measured depth.
- **The harness crate is never shipped.** Like the determinism bench, it is
  evidence tooling in the workspace.

## Consequences

### Positive

- U1 (AGENTS.md) gains an instrument: one command, one verdict, per pair of
  hosts.
- The longest sim conformance run in the tree is now a committed, replayable
  artifact.

### Negative

- The committed log depends on the installed data through its two digests; a
  game patch invalidates it until the ignored generator is run again.
- The recursion depth is measured, not bounded, so a plugin in the tick loop
  remains blocked on a deterministic depth bound.

## Verification

- DET-030: `sim_replay_start_refuses_a_wrong_header`.
- DET-027: `conformance_sim_replay_digests_equal`, and the two-process
  check `conformance_sim_replay_two_processes_agree`.
- DET-028: `conformance_sim_replay_diverges_at_altered_tick`.
- DET-029: `conformance_sim_replay_covers_manifest`.
- DET-024: `conformance_sim_replay_cases_are_repeatable`,
  `conformance_sim_replay_report_shape` (the compared lines and the NaN
  placement), and `tools/replay_compare.sh`.
- The generator: `conformance_sim_replay_generator_reproduces_log`.
