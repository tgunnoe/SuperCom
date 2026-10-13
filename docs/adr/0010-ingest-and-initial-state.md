# ADR-0010: Ingesting the static world and the initial state

**Status:** PROPOSED

**Date:** 2026-10-12

## Context

The simulation core (ADR-0008) holds a state and runs ticks, but no rule can
act until the sim has a world to act in: the unit and projectile kinds of the
blueprint catalog, the map's terrain, and the scenario's markers. The sim
crate's lint forbids the filesystem, the environment, and the clock (SIM-036,
DET-001), so the world must arrive as parsed values, and a second crate must
read the files. Three questions have no answer in the installed game or the
reference stubs: how a cell's slope is computed and what makes a land cell
passable, what an army starts with, and what identifies a world in a replay.
This record lists the choices by clause so the specification's design clauses
have a ratifiable source (AGENTS.md R2, R7).

## Decision

- **Two crates (SIM-049).** `crates/sim` holds the world's types and builds a
  `World` from parsed values only; `crates/sim-ingest` reads the catalog, the
  map, and the save file and converts them. The conversion of the map
  parser's and the save parser's values into the sim's own types is
  `crates/sim-ingest`'s, so the sim depends on the typed blueprint model and
  on nothing that reads a file.
- **Closed sets (SIM-049).** Motion types, layer keys, command capabilities,
  and build restrictions are read into enumerations, and an unknown value is
  an ingest error. The installed data uses the eight motion types of SIM-029,
  the six layer keys of SIM-030, the 21 capabilities of SIM-016, and two
  build restrictions (mass deposit on 12 units, hydrocarbon deposit on 4).
  Refusal is this project's choice: the original's behaviour on an unknown
  value is unobserved, and a closed set makes a mistyped modded value a load
  error rather than a unit that silently cannot move. Each builder's
  buildable set is resolved once at ingest by SIM-011's rule, so the tick
  never evaluates category expressions.
- **Rounding (SIM-050).** Each blueprint scalar the sim holds as `I32F32`
  converts once, to the nearest representable value, through the checked
  conversion; a value of magnitude 2^31 or more, or not finite, is an ingest
  error naming the field's path. The largest installed value is 10,008,000
  (a build cost), well inside the range. Heightmap samples convert exactly:
  the stored scale is 2^-7 on every installed map, and the loader requires a
  power of two no smaller than 2^-16. Geometry stays `f64` (ADR-0006).
- **Slope and land passability (SIM-048).** A cell's slope is the maximum
  absolute height difference over its four edges between adjacent corner
  samples, never a diagonal, and a cell is land-passable when that slope does
  not exceed 0.75, the `MaxSlope` of every land, amphibious, and water-land
  footprint (SIM-047). This is the approximation of the two community map
  tools, adopted pending an observation of the original: no readable source
  states the engine's slope function, and the engine header names `MaxSlope`
  without a pathing structure. The slope condition is necessary, never
  sufficient: later clauses may add conditions (water depth, blocking
  entities), and none may remove it. Air is passable everywhere. The water,
  seabed, and sub grids are absent until a clause derives them from water
  elevation, which the map loader does not yet decode.
- **Starting resources (SIM-051).** A skirmish starts each army with stored
  mass and energy equal to its commander's storage values, 650 and 4000 for
  the four installed commanders. This is a design choice: the original sets
  an army's starting resources in readable Lua whose call sites are not yet
  read, and the value is an observation candidate. Slot `n`'s commander is
  placed at the marker named `ARMY_n`, at the marker's position.
- **The catalog digest.** The replay header carries a catalog digest and a
  map digest (DET-007). The catalog digest is SHA-256 over a canonical
  encoding of every unit and projectile kind in id order, in little-endian
  fixed-width fields, documented on the digest function; it therefore covers
  exactly what the sim reads, not the files they came from, so a cosmetic
  change to a blueprint file does not change it and a change to a value the
  sim reads does. The map digest covers the map name, the grid, and the
  markers, because the markers place the armies.
- **The world outside S (SIM-036).** The state holds its world by shared
  reference, set at construction and never reassigned, and the world is not
  serialized: it is identified by the two digests, so the serialized state S
  (DET-009) and the core's golden vectors are unchanged. A tick is a pure
  function of the state, its world, and the command batch.

## Consequences

### Positive

- Every file read lives in one crate, and the sim's lint keeps its
  guarantee: no filesystem, environment, or clock inside `crates/sim`.
- The digests make a replay refuse a different catalog or map before it runs.
- Rules of later families read closed enumerations and `Q` values, with no
  parsing or conversion at tick time.

### Negative

- The sim depends on the typed model crate and so, transitively, on the
  Nickel evaluator, for one function's parameter types.
- The slope function and the land rule may disagree with the original's;
  the first observation of it reopens SIM-048.
- A catalog digest tied to the sim's own field list changes whenever a field
  is added; replays recorded before the change fail the content check.

## Open

The water, seabed, and sub passability grids wait on water-elevation
decoding in the map loader. The original's starting resources and its slope
function remain unobserved.

## Alternatives Considered

- A digest over the blueprint files' bytes: rejected, because it would tie a
  replay to formatting the sim never reads and would require the sim to see
  the files.
- A diagonal term in the slope: rejected as the unattested option; the two
  community tools use edge differences, and the choice is recorded for
  revision.
- An unknown value passed through as a string: rejected, because a rule would
  then have to handle values the sim cannot reason about.
