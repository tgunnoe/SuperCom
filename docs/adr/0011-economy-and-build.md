# ADR-0011: The economy and build rules

**Status:** PROPOSED

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
  refused until the water layers exist.
- **Refusals count as ignored commands (SIM-061).** An order that fails a
  kind or site check is dropped when it reaches the head of the queue and
  counted in the commands-ignored counter; an order out of range waits.
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

- Every rule is fixed-point and visits entities in ascending id, so the
  economy and build phases keep the tick a pure function of the state.
- The ledger and the counters make a stall observable to readers without a
  second source of truth.

### Negative

- The lesser-of-two fraction and the sum of builders may disagree with the
  original's arithmetic; the first observation reopens SIM-040 and SIM-059.
- Crediting complete units only differs from the installed Lua's activation at
  creation, which may matter for a structure's partial output.

## Open

Debts: resuming an unfinished structure after a Stop, and buffs and
modifiers, which are 0 and 1 in this version. Starting resources remain
SIM-051's recorded departure.

## Alternatives Considered

- A global per-army fraction for every consumer: rejected, because a unit
  short of one resource would then be starved of the other, against the
  installed cancellation of production by the consumed fraction.
- Floating-point rates with a 0.1 literal: rejected, because rates, costs,
  and progress are simulation state and the float strategy keeps them
  fixed-point (ADR-0006).
- Crediting production from creation, as the installed Lua activates it:
  rejected for this version, because the engine's treatment of an unfinished
  unit is unobserved and the conservative reading cannot overstate income.
