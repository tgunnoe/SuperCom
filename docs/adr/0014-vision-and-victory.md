# ADR-0014: Vision and victory: the intel and victory phases

**Status:** ACCEPTED (ratified by the head on 2026-10-19 at the pull request that landed it)

**Date:** 2026-10-18

## Context

The tick loop reserves the last two phases for intel and victory (ADR-0008)
and the combat rules remove a dead entity without deciding anything about
the army that owned it (ADR-0013). The installed game states what the
unit blueprints carry for vision (SIM-032, SIM-082) and how its victory
script judges an army (SIM-034, SIM-082), but not how the engine turns a
vision radius into what an army sees, nor what the script's callers do
beyond marking a brain out of the game. This record lists the choices by
clause so the design clauses have a ratifiable source (AGENTS.md R2, R7).

## Decision

- **Vision is a circle of the blueprint's VisionRadius on the horizontal
  plane (SIM-083).** An army sees its own entities and every other entity
  within a complete seeing entity's radius, inclusive, Euclidean in x and z.
  The engine's vision grid and its resolution are unreadable, so no grid is
  modelled and the renderer's fog of war is built on the sets this phase
  exposes. Radar, sonar, omni, stealth, cloak, jamming, and water vision are
  recorded debts, as are intel toggles and energy-driven shutdown. 4 of the
  395 blueprints with an Intel block have no VisionRadius and see nothing
  (SIM-082).
- **The visible sets are derived and live outside the serialized state
  (SIM-083, DET-009).** They are a pure function of the positions, the
  finished flags, and the kinds, rebuilt from scratch by every intel phase,
  as the route planner's call counter is derived and never hashed. Two
  states that agree on S agree on the sets, so hashing them would add bytes
  and no information.
- **Weapon acquisition does not consult vision in this version
  (SIM-075).** A weapon takes the nearest enemy in range whether or not its
  army sees it. The original's rule is unreadable and the combat family is
  frozen; consulting the sets later is a change to SIM-075 and its tests.
- **Demoralization is the one victory option (SIM-084).** The script names
  it first, and it is the skirmish default. An army with no complete
  commander is defeated in the victory phase, in slot order. A commander still
  under construction does not count. Domination and eradication, alliances,
  allied victory, draw offers, civilian armies, and an end-of-game timer are
  recorded debts.
- **A defeated army's entities persist and its commands are ignored
  (SIM-084).** The script's defeat only marks the army out of the game; it
  removes no unit. The commands phase therefore ignores and counts, as a
  foreign command, every command of a defeated army from the tick after the
  defeat.
- **A world with no COMMAND kind defeats nobody (SIM-084).** The original
  would defeat every army with a zero count; this project's synthetic worlds
  for the economy, build, movement, and combat families contain no commander
  kind and several armies, and a real world always has them. The condition is
  a precondition, not a rule about armies. The script under demoralization
  would draw such a world at its first check (its line 16 is the branch for
  any other option, which sets no condition). The precondition retires when a
  second victory option lands: the option becomes an explicit match setting
  and the catalog check is cut.
- **The outcome is decided once (SIM-085).** The first victory phase after
  which at most one army stands names Winner of that army (counted in
  `victories`) or Draw (not counted). Afterwards the phase does nothing and
  the ticks keep running. A state of fewer than two armies decides nothing,
  defeats included.
- **The serialized state grows (DET-009).** Two counters follow deaths,
  `defeats` then `victories`, and the outcome follows the projectiles: a byte
  0, or 1 then a tag byte (0 and the winning army's slot byte for Winner, 1
  for Draw). The visible sets are not serialized. The catalog digest does
  not change.

## Consequences

### Positive

- A skirmish match ends: killing the last commander of an army defeats it,
  and the last army standing is declared the winner, to the tick.
- The renderer and the AI read what an army sees from `State::visible`.

### Negative

- Vision is a circle without terrain occlusion, resolution, or intel kinds
  other than sight, so it will drift from the original's feel; the first
  observation reopens it.
- Under demoralization an army that loses its commander loses at once even
  while it holds a factory that could build another.
- Acquisition ignoring vision lets a unit shoot at what its army cannot see.

## Open

- The engine's vision arithmetic and grid resolution.
- Whether the engine's acquisition requires the target to be visible.
- The other two victory options and the allied-victory and draw-offer paths.
- The intel cost: armies x entities x seers per tick (`run_intel`),
  unmeasured. Signpost: a tick-time measurement at the unit cap. The table
  is derived and outside the serialized state, so a spatial index or
  per-army seer lists would change no digest.

## Alternatives Considered

- Hashing the visible sets into the state: rejected, since they are derived
  from S and would only enlarge every digest.
- Defeating an army only when it has lost every unit: rejected as the
  eradication option, not the skirmish default.
- Removing a defeated army's entities: rejected, since the script marks the
  army out of the game and removes nothing.
- Re-pinning the synthetic multi-army test worlds with a commander kind:
  rejected, since it would put a commander into every later multi-army test
  for no rule the test checks; the precondition above stays instead.
- Victory option as an explicit match setting now: deferred to the arrival
  of a second option, when the setting has something to select between.
