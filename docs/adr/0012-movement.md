# ADR-0012: Movement, pathing, and the water layers

**Status:** ACCEPTED (ratified by the head on 2026-10-18 at the pull request that landed it)

**Date:** 2026-10-13

## Context

The simulation core runs a movement phase after the build phase in every
tick (ADR-0008), but the phase does nothing, so no unit moves and an assister
cannot approach its builder. The installed game states
which motion type each blueprint has, which speed, turn rate, acceleration,
and brake it carries, and the footprint table's layer caps and depth bounds
(SIM-062, SIM-063). It states nothing of how the engine integrates a unit's
motion, steers it, or plans a path: no installed Lua reads the physics
fields, and the engine stubs document only that a velocity is in ogrids per
tick. This record lists the choices by clause so the design clauses have a
ratifiable source (AGENTS.md R2, R7).

## Decision

- **A cell is one world unit, and a layer is assigned each phase
  (SIM-064).** The heightmap's cell is the unit of the passability grids and
  of routes. A mobile entity's layer is set every movement phase from its
  kind's motion type and, on a map with water, the terrain under it; an
  unknown blueprint, an immobile kind, and a kind without a maximum speed are
  skipped whole, so the core's command tests on an empty world keep their
  queues. A land entity stands on the terrain, an air or hover entity above it
  by the kind's elevation.
- **The integration formula and its order of operations (SIM-065).** Heading
  turns toward the next target at the kind's turn rate, speed rises by the
  acceleration and falls by the brake inside the braking distance, and the
  position advances along the new heading, in the exact binary64 operations
  the clause lists, with sine, cosine, arc tangent, and square root from libm
  (DET-018 to DET-020). The standard library's transcendental methods are
  forbidden to the sim. No readable source states the engine's integration or
  steering (SIM-062), so every term is this project's choice; the first
  observation of a unit's speed, heading, or turn reopens the clause.
- **Instant defaults for absent fields, and the Air block unread
  (SIM-063, SIM-065).** The census finds 47 of the 60 air units without a
  turn rate or an acceleration and 71 movers without a brake, so a unit
  without a turn rate turns at once and without an acceleration reaches its
  maximum speed at once. A unit without a brake takes its acceleration as the
  brake rate, and one with neither stops at once whenever a braking rule
  applies (below). These are deviations, not readings. The Air block (MaxAirspeed, TurnSpeed, KTurn) is
  outside the typed model and is not read, so aircraft fly as other movers at
  their Physics speed over the Air layer.
- **The planner (SIM-066, SIM-067).** A* over the eight-neighbour lattice
  with costs of 10 and 14, a diagonal step only when both orthogonal cells it
  cuts are passable (no corner cutting), the heuristic 10 times the larger
  axis distance plus 4 times the smaller, and ties broken by the larger cost so far
  and then the ascending cell index, with the earlier parent kept on equal
  cost, so the planner is a pure function (SIM-033). Among equal estimates the
  cell furthest along is expanded first because the other order sweeps the
  whole band of equal-cost routes on open ground: one open plan took 22.6,
  88.7, and 394.7 ms on grids of 1024, 2048, and 4096 cells a side in
  release, where preferring the larger cost so far took 1.5, 4.2, and 17.4
  ms for routes of the same length. This is the project's first planner; the original's is
  unreadable. Routes are not string-pulled, so a route is a chain of
  adjacent cells and a unit follows it cell centre to cell centre.
- **A unit that is still turning brakes (SIM-065).** No readable source
  states the engine's steering. A unit whose waypoint lies inside its turning
  circle must shed speed or never arrive, as the installed naval unit shows
  on its first move: full speed, a turn radius of several units, and a target
  one unit away. So a unit with a brake that has not yet reached the desired
  heading brakes by its brake rate, and the braking-distance and acceleration
  rules apply once it faces its target. A unit is still turning when its
  step was clamped by the turn rate. A kind without a brake brakes at its
  acceleration, and a kind with neither brakes to a stop at once, whatever
  its motion type: seven ground kinds carry no MaxBrake, the four commanders
  among them, and circled under the earlier default of never braking. An air
  kind with neither field therefore stops at once when it must brake; the
  original's air flight is unobserved.
- **Arrival, follow range, and stopping at a cliff (SIM-065, SIM-068).** A
  cell centre or a goal counts as reached within 0.5 units, compared as
  squares so no root is taken; a guard follows to 5 units, the build range;
  and a unit whose step would enter an impassable or absent cell keeps its
  place with speed 0 and its new heading, never clipping through a cliff.
- **Units pass through one another (SIM-068).** There is no collision, push,
  or avoidance in this version; the build phase already refuses a site over a
  mobile unit's cell, and nothing else reads two units' positions.
- **A Stop is one helper (SIM-068).** The build phase and the movement phase
  both call it, so a Stop at the head of a builder's queue and at the head of a
  mover's agree: it clears the work, the route and the goal, sets the speed to
  0, and pops.
- **Guard follows and assists (SIM-068).** A guard farther than the follow
  range plans a route to the target's cell, or to the first passable of its
  eight neighbours, and plans again only when the target's cell changes; it
  keeps the target's cell it planned for in its goal, so a guard of a target
  it cannot reach holds its order at speed 0 with an empty route and does not
  search again every tick (a failed search of a split grid the size of a
  20 km map cost 52.2 ms a tick in release). Within the follow range of the
  target's position it stops and keeps the order. The build phase's assist rule is unchanged, so a
  guard with a build rate both follows and assists from the position the
  movement phase left.
- **Time (SIM-069).** The integration's dt is the tick's tenth of a second;
  speeds are stated per second in the blueprints, and the stubs' ogrids per
  tick are read as the derived per-tick advance.
- **The water layers (SIM-070).** The world reads the map's water surface and
  nothing of its deep and abyss elevations: the engine's Lua surface exposes
  GetSurfaceHeight, which takes water into account, and GetTerrainHeight, and
  mentions neither, so whether the original reads them is unobserved. A cell's
  depth is the surface less its corner height; each layer takes one threshold,
  the bound of the footprint rows its motion types use most (SIM-062), and the
  rest are deviations: Land adds a depth bound of 0.05 (the eight
  amphibious-floating kinds' WaterLand rows bound depth at 0.1 to 1 and 0 to
  5, and both are deviated from: those kinds move on the Land-passable cells
  and every cell of depth above 0.05, as the hover unit does, because the
  union of Land and Water leaves a gap between 0.05 and 1.5 that a gradual
  shore always falls in, and on 28 of the 40 installed maps with water cells
  it had no step across at all), Seabed is depth above 0 and
  at most 25 with the slope bound, Water and Sub are depth at least 1.5 (the
  row of 0.25 is deviated from), and a hover unit takes Water over any depth
  above 0.05 because no hover row exists. Water presence does not imply a
  submerged cell. The map digest writes the surface as an option after the
  height samples, which changes that one place of the documented layout.
- **The serialized state's appended fields (DET-009).** The counters gain
  moves completed; each entity its heading, speed, layer, route, goal, and
  maintenance fraction, in the order documented on the serialization
  function, and the two tests that pin bytes are re-pinned to it.

## Consequences

### Positive

- Units move, guards approach their targets, and the build phase's range check
  sees real positions, all inside the deterministic tick: every rule visits
  entities in ascending id and uses only exact operations and libm.
- The planner is a pure function of its inputs, so replays agree.

### Negative

- An air kind without a brake or an acceleration stops at once when it
  must brake, which the original's air flight, unobserved, may not do.
- Units pass through one another, so a crowd occupies one cell.
- The integration, the planner, and the thresholds may disagree with the
  original's; the first observation reopens SIM-065, SIM-066, and SIM-070.
- Routes follow a lattice and bend at every cell, where the original's units
  move more directly, and each bend of a route brakes a turn-limited unit:
  the conformance tank spent 41 of the 525 ticks of a 117-cell route braking
  at bends.
- A build order whose site is out of range waits where the builder stands, so
  a Move must precede it; nothing yet walks a builder to its site, a debt for
  the AI family.
- A guard stops at the follow range of its target's position, not of the site
  its builder works on, so a guard that approaches from the far side of its
  builder from the site may stop outside the site's build range and never
  assist. The follow rule is kept; walking an assister to build range is a
  debt.

## Open

Debts: a builder that walks to its site and an assister that walks to the
site's build range, string-pulling of routes, unit avoidance and collision,
formations,
transport, patrol, dive and dock, fuel, banking and pitch, golden vectors for
libm's cosine, arc tangent, and square root (DET-019 and DET-020 cover the
sine and the exponential), and the engine's observed integration.

## Alternatives Considered

- Fixed-point positions: rejected, because the economy is fixed-point by
  ADR-0006 for state arithmetic while geometry stays binary64 under exact
  operations, and a position needs the transcendental functions.
- A navigation mesh or a hierarchical planner: rejected for this version; a
  lattice A* is the smallest deterministic planner and the grids already exist.
- Deriving every layer's grid from its own footprint row: rejected, because
  the sim's passability is per layer while the table's bounds are per row.
