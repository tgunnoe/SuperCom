# ADR-0014: Vision and victory: the intel and victory phases

**Status:** ACCEPTED (ratified by the head on 2026-10-22, explicitly, after the review that landed it, amended on 2026-10-24 for the defeat kill rule and the civilian exemption, the amendment ratified by the head on 2026-10-29 at pull request 58)

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
- **A defeated army's entities are killed 200 ticks after the defeat
  (SIM-084, SIM-087).** The script's defeat marks the army out of the game
  and, 20 seconds later, kills every unit of the army outside the WALL
  category (lua/aibrain.lua of lua.scd, lines 787 to 794). The army records
  the tick T whose victory phase defeated it, and the death phase of tick
  T + 200, after its own removals, removes every entity of that army whose
  kind is not WALL (a kind absent from the world counts as not WALL), in
  slot order and ascending id, through the helper the death phase uses for
  its own removals, so an Attack at the head of another entity's queue that
  names a killed entity is popped. The kill
  sits in the death phase because the visible sets are rebuilt by the intel
  phase after it and so stay consistent, the victory phase's freeze after an
  outcome stands (in a two-army match the kill follows the outcome), and the
  death phase stays the one remover of entities. The script lists the army's
  units by a call whose treatment of unfinished units is unreadable; this
  version reads it as killing them, finished or not. Kills are counted in
  `kills`, apart from deaths. The commands phase ignores and counts, as a
  foreign command, every command of a defeated army from the tick after the
  defeat.
- **A civilian army is never defeated and never counted (SIM-084,
  SIM-085).** The script skips civilian brains in its defeat loop
  (lua/victory.lua of lua.scd, line 28). Excluding them from the standing
  count, and requiring two non-civilian armies before the phase judges, is
  this project's reading of that skip. Only `State::add_civilian_army`
  creates a civilian army, and nothing changes the mark afterwards.
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
  the ticks keep running. A state of fewer than two non-civilian armies
  decides nothing, defeats included.
- **The serialized state grows (DET-009).** Three counters follow deaths,
  `defeats`, `victories`, then `kills`; each army carries, after its four
  amounts, the defeat as a byte 0, or a byte 1 then the defeat tick as a
  little-endian 64-bit integer, and then a civilian byte 0 or 1; and the
  outcome follows the projectiles: a byte 0, or 1 then a tag byte (0 and the
  winning army's slot byte for Winner, 1 for Draw). The visible sets are not serialized. The catalog digest does
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
- Reading the scenario's armies: no match setup in this version creates a
  civilian army, since the civilian flag is match setup absent from every
  map file. Issue 56 carries it.
- The intel cost: armies x entities x seers per tick (`run_intel`),
  unmeasured. Signpost: a tick-time measurement at the unit cap. The table
  is derived and outside the serialized state, so a spatial index or
  per-army seer lists would change no digest.

## Alternatives Considered

- Hashing the visible sets into the state: rejected, since they are derived
  from S and would only enlarge every digest.
- Defeating an army only when it has lost every unit: rejected as the
  eradication option, not the skirmish default.
- Removing a defeated army's entities in the victory phase: rejected, since
  the victory phase would then remove entities after the intel phase has
  built the visible sets, leaving them naming removed entities until the
  next tick, would make a second remover beside the death phase, and in a two-army match it would act after the outcome, which SIM-085's freeze forbids.
- Writing a defeated army's units' health to zero at the defeat's expiry and
  letting the death phase find them: rejected, since an unfinished entity
  is removed by the death phase only when the damage phase marks it, and the kill would need a second marking path through a phase that has already run; it would also leave a complete entity at zero health across a tick boundary, count the kill as a death, and land one tick late.
- Re-pinning the synthetic multi-army test worlds with a commander kind:
  rejected, since it would put a commander into every later multi-army test
  for no rule the test checks; the precondition above stays instead.
- Victory option as an explicit match setting now: deferred to the arrival
  of a second option, when the setting has something to select between.
