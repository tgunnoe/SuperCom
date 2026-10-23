# ADR-0011: The economy and build rules

**Status:** ACCEPTED (ratified by the head on 2026-10-18 at the pull request that landed it)

**Date:** 2026-10-13

## Context

The simulation core runs ten phases per tick (ADR-0008), and the world seam
gives it unit kinds, a map, and markers (ADR-0010), but the economy and build
phases do nothing. The installed Lua states the build-cost formula, how
production and maintenance are set, and how an extractor reads its marker
(SIM-052 to SIM-054), and the blueprint census fixes the data the rules read
(SIM-055). It does not state how several consumers share a shortage, how
several builders sum, where a factory's unit appears, or how a site is
checked, and the engine's own arithmetic is not readable. This record lists
the choices by clause so the design clauses have a ratifiable source
(AGENTS.md R2, R7).

## Decision

- **The stall rule and the lesser-of-two fraction (SIM-040).** Requests are
  settled per army against stored plus income. The fraction of a resource
  granted is the lesser of 1 and available over requested; an entity
  requesting both resources takes the lower fraction for both, so a unit short
  of one resource consumes the other at the same fraction. The engine's
  arithmetic is unreadable and its behaviour under a shortage unobserved.
- **A producer's fraction lags one tick (SIM-040, SIM-043).** A unit's
  production in a tick is scaled by the fraction granted in the previous
  economy phase, because the settlement needs the income to know what is
  available. The installed cancellation code shows the engine scales
  production by the consumed fraction (SIM-053).
- **Hydrocarbon placement mirrors the extractor rule (SIM-042).** No
  installed Lua states a marker rule for a hydrocarbon plant (SIM-054), so a
  plant is placed within 1 unit of a Hydrocarbon marker no other plant holds,
  as an extractor is on a Mass marker (SIM-041).
- **Storage as a sum, and the clamp (SIM-058).** An army's storage is the sum
  of its complete units' kind storage, and the stored amounts are clamped to
  it after each settlement.
- **Production active from completion (SIM-058).** The installed Lua
  activates production at a unit's creation (Unit.lua line 73 sets
  EconomyProductionInitiallyActive and line 173 reads it, SIM-053), but what
  the engine credits for an unfinished unit is unobserved, so production and
  storage are credited only from complete units.
- **Progress and assist as the sum of contributors' rates (SIM-059).** The
  work an entity under construction gains in a tick is the sum of its
  contributors' granted fractions times their build rate per tick; no
  readable code states a sum over several builders.
- **Health tracks progress (SIM-059).** An unfinished entity's health is its
  maximum times its progress, until the combat family refines it.
- **A factory spawns at its position (SIM-060).** The unit under construction
  appears at the factory's position and the queue advances one item at a
  time.
- **Footprint default (SIM-061).** The footprint defaults to one cell for the
  205 blueprints that read as none (166 without a Footprint block, 39 with a
  block that states no SizeX), and a zero dimension reads as 1.
- **The build range is 5 for every builder (SIM-061).** The ten stated
  MaxBuildDistance values (10 on four commanders, 5, 100, 15, 20, 25, and 1)
  are a deviation, and MaxGroundVariation (15 blueprints, all 50) is not
  applied.
- **No water placement (SIM-061).** A kind whose build layers lack land is
  refused in this version although the water layers exist (SIM-070);
  placement on water is a recorded debt.
- **Refusals count as ignored commands (SIM-061).** An order that fails a
  kind or site check is dropped when it reaches the head of the queue and
  counted in the commands-ignored counter; an order out of range waits.
- **Placement and range are `f64` geometry (SIM-061).** Positions and
  distances are geometry under ADR-0006's exact operations; everything
  economic is fixed-point. The site's height is the corner sample of the
  map cell under the order's position, not the order's own height.
- **The completing tick charges a whole tick (SIM-059).** The request is
  settled before the build phase knows how little work remains, so the tick
  that completes a build charges and grants a full tick's rates. The
  overcharge measured over the installed builder-kind pairs exceeds 1 percent
  on 75 of 1,220 pairs and reaches 20 percent at worst. Charging only the
  remainder is open.
- **A producer's output is scaled by its maintenance fraction only
  (SIM-043).** The head ruled that a unit's production is multiplied by the
  fraction granted to its maintenance request, 1 when it makes none, and never
  by the fraction of a build request it makes. The installed cancellation
  (SIM-053) shows the engine scales a consuming producer, and the ten
  commanders carry Economy.NaturalProducer (SIM-055): a producer scaled by its
  own build request would fall to zero income at empty stores and freeze its
  build, which an army starting from the installed save files' zeros would meet
  on its first build (SIM-051 instead starts each army at its commander's
  storage, the recorded departure). A mass fabricator still keeps
  requesting its maintenance energy under a shortage. The economy phase
  records each entity's maintenance fraction from that request's resources
  alone, and the granted fraction keeps its meaning for the build phase.
- **An army without storage is clamped to 0 (SIM-058).** The clamp takes the
  sum of complete units' storage, so an army with none holds nothing after a
  settlement.
- **Mobile units occupy their cell, the builder included (SIM-061).** A site
  over any mobile unit's cell is refused, including the cell of the builder
  that ordered it.
- **A replacing command clears work as a Stop does (SIM-059 to SIM-061).**
  The replaced target stops gaining and costing from that tick and stays in
  place unfinished; a work item naming an entity that is gone or finished is
  cleared at the start of the build phase.
- **Only complete entities act (SIM-059, SIM-061).** An unfinished entity
  starts no build, takes no Guard assignment, adds no work, and requests
  nothing; its commands queue and wait. Any complete kind with a build rate
  can assist a build, whether or not it can build the kind itself.
- **An assist counts from the build phase after its Guard is assigned
  (SIM-059).** The assignment is made at the end of a build phase, so the
  assister's first contribution is the next one.
- **The serialized state's appended fields (DET-009).** The counters gain
  builds completed and stall ticks; each army its ledger of eight values;
  each entity its work done, completion flag, fraction, and work item, in the
  order documented on the serialization function, and the one test that pins a
  minimal state's bytes is re-pinned to it.
- **Fixed-point division by 10 (SIM-057).** Every per-second rate applies per
  tick as the value divided by the integer 10 in `I32F32`; the build-time
  floor is the separate constant of raw bits 429496730, the nearest value to
  0.1 (SIM-050), which differs from one tenth of 1 by a raw unit.

## Consequences

### Positive

- All economic quantities are fixed-point and every rule visits entities in
  ascending id, so the economy and build phases keep the tick a pure
  function of the state.
- The ledger and the counters make a stall observable to readers without a
  second source of truth.

### Negative

- The lesser-of-two fraction and the sum of builders may disagree with the
  original's arithmetic; the first observation reopens SIM-040 and SIM-059.
- Crediting complete units only differs from the installed Lua's activation at
  creation, which may matter for a structure's partial output.

## Open

Debts: resuming an unfinished structure after a Stop or a replacing
command, and buffs and
modifiers, which are 0 and 1 in this version. Starting resources remain
SIM-051's recorded departure.

## Alternatives Considered

- A global per-army fraction for every consumer: rejected, because a mass
  fabricator, which consumes only energy, would then be throttled by a mass
  shortage it takes no part in, against the installed cancellation of
  production by the consumed fraction.
- Floating-point rates with a 0.1 literal: rejected, because rates, costs,
  and progress are simulation state and the float strategy keeps them
  fixed-point (ADR-0006).
- Crediting production from creation, as the installed Lua activates it:
  rejected for this version, because the engine's treatment of an unfinished
  unit is unobserved and the conservative reading cannot overstate income.
