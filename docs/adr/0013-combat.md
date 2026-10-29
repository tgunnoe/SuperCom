# ADR-0013: Combat: weapons, projectiles, damage, and death

**Status:** ACCEPTED (ratified by the head on 2026-10-18, in one ruling over the six simulation records, amended on 2026-10-24 for the minimum-radius back-off, the amendment ratified by the head on 2026-10-29 at pull request 58)

**Date:** 2026-10-14

## Context

The tick loop reserves four phases after movement for combat (ADR-0008) and
the movement rules leave an Attack order queued untouched (ADR-0012). The
installed game states the weapon and projectile fields (SIM-071, SIM-072),
the armour table and the damage path (SIM-074), and the shape of the firing
cycle, but not how the engine acquires targets, composes the fire clock with
the salvo and reload waits, flies a projectile, or whether the amount that
reaches the damage function is before or after the armour multiplier. This
record lists the choices by clause so the design clauses have a ratifiable
source (AGENTS.md R2, R7).

## Decision

- **Acquisition is the nearest enemy, by id on ties (SIM-075).** A weapon takes
  the Attack's target, else a target it holds, else the nearest enemy in range
  on an allowed layer. The engine's acquisition is unreadable; TargetPriorities
  (on 337 of 494 weapons), TrackingRadius, TargetCheckInterval, and
  FiringTolerance are not read, which is a deviation with the census of
  SIM-071.
- **Active weapons (SIM-075).** A weapon of category Death or Kamikaze, a
  manual-fire weapon, a weapon an enhancement enables, and a weapon without a
  rate of fire never fires. Shields (SIM-024's state machine) and veterancy
  are recorded debts, not modelled.
- **The fire clock is ceil(10 / rate) ticks and one projectile per shot
  (SIM-076).** The installed cycle (SIM-071) multiplies racks, muzzles,
  MuzzleSalvoDelay, and a reload wait; how the engine composes them with the
  RateOfFire throttle is unreadable, so MuzzleSalvoSize, MuzzleSalvoDelay,
  RackSalvoReloadTime, RackSalvoChargeTime, and ProjectilesPerOnFire are
  deviated from. A weapon without a projectile kind or a speed makes an
  instant hit.
- **The flight model follows the projectile kind's flags (SIM-077).** BallisticArc
  is not read; a kind that falls launches at the lower solution of the
  ballistic equation for the planar distance, with the default gravity of -4.9
  that the engine stub documents and no installed Lua sets (SIM-073). A
  projectile impacts when it comes within the step plus 0.5 of its target,
  because a step of 2.5 units at speed 25 can jump over a sphere of radius
  0.5. The position moves by the mean of the old and new velocity, which is exact
  for a constant acceleration; moving by the new velocity alone (semi-implicit
  Euler) sinks below the parabola in proportion to the flight time, so no fixed
  ground tolerance covers long flights. The ground rule is then a strict test
  against the terrain height. Shots beyond the ballistic maximum are launched at
  45 degrees and miss (see Consequences).
  A projectile
  leaves the map when its position is outside the grid, not only when its cell
  is absent.
- **Damage is the multiplier at impact, uniform in the radius, to enemies
  only (SIM-078).** Whether the amount reaching the engine's damage function
  is before or after the armour multiplier is unverified (SIM-074), so the
  multiplier is applied at impact. The default multiplier is 1 for an armour
  or a damage type outside the table, since the set of damage types is open
  (SIM-023). Radius damage has no fall-off: no readable source states one.
  A ground impact damages through the radius alone. Damage to a site under
  construction is lost work, so its health and progress stay consistent.
- **Regeneration is a tenth of the rate each tick for a living complete
  entity (SIM-079).** An entity at or below 0 health is never healed.
- **Death is removal at or below 0 health (SIM-080).** There is no wreckage
  (SIM-027 is a debt), no death weapon, no overkill ratio (the cited path
  computes one), and no army defeat, which belongs to the victory family.
- **The Attack order is the guard rule to the longest weapon range, then fire
  (SIM-081), and a mobile attacker inside its minimum radius backs off
  first (SIM-086).** The band floor is the smallest positive minimum radius
  among the active weapons that carry a maximum radius, defined only when
  every such weapon has a positive minimum and that minimum is below the
  longest range. Below the floor the attacker's goal is the point on the line
  from the target through it at the floor plus a margin of one world unit
  (in +x when the two coincide); the margin only places the goal's cell
  beyond the floor. The decision is made afresh every tick, so the attacker
  stops at the first tick its distance reaches the floor, where the approach holds it; that is in general short of the goal, and at the goal when a step reaches it. The Attack is never popped. When the goal
  has no cell on the map, is impassable, or has no route, the attacker holds
  with speed 0 and keeps the Attack; no neighbouring cell is substituted,
  since one can lie inside the floor. An attacker the movement phase skips
  never moves and fires once the target leaves its minimum, and a kind with
  an active weapon that has no minimum has no floor and never backs off.
  The installed data has 40 kinds whose every ranged weapon carries a
  minimum: artillery and structure artillery (minimum 5 to 175), missile and
  torpedo kinds (3 to 20), the commanders and sub-commanders (1 or 2 against
  a maximum of 22 to 30), and one Aeon experimental (2 against 40). The
  original engine's handling is unreadable, so the rule is this project's
  design.
- **Two per-tick values live outside the state.** The impacts the projectiles
  phase hands to the damage phase and the set of unfinished entities marked for
  death are created by the step and passed to the phases; nothing survives a
  step by construction.
- **The serialized state and the catalog digest grow (DET-009).** The state
  gains two counters, each entity's weapons, the next projectile id, and the
  projectiles; the catalog digest gains each weapon's category and enhancement
  flag and the armour table.
- **The armour table is read at ingest from the installed Lua.** The sim-ingest
  crate reads lua/armordefinition.lua through the archive layer's mount order
  and passes it to World::from_parts_with_armour. That table is read outside
  the Nickel overlays of ADR-0002, which cover the blueprint families only;
  the table parameter of from_parts_with_armour is where a typed-model source
  would plug in. The enhancement flag is read from the typed model's
  EnabledByEnhancement, so an enhancement-gated weapon stays inert until an
  enhancement rule exists.

## Consequences

### Positive

- A skirmish match can fight: units acquire, fire, hit, and die, and the
  conformance test kills a structure on a real map to the tick.
- Every rule is a clause with a test, and the digests pin the layout.

### Negative

- Acquisition, the clock, and the flight model are this project's reading and
  will drift from the original's feel; the first observation reopens them.
- Salvo weapons fire one projectile per shot, so burst weapons are weaker.
- A falling shell hits at every range up to and at its speed squared over 4.9 on flat ground; a target beyond that is launched at 45 degrees and missed, which 16 of the 367 active installed weapons can be ordered to do (8 Anti Navy, 8 Defense, no Artillery); the elevation uses the planar distance only, so a target at another height is reached at the firer's height.

## Open

- Whether the engine applies the armour multiplier before the damage function.
- The engine's acquisition, salvo clock, and flight integration.
- Shields, veterancy, wreckage, and beam weapons.

## Alternatives Considered

- Recording the limitation and naming the classes instead of moving: rejected,
  since the commanders at minimum radius 1 and the artillery the built-in AI
  will order to attack would stand inside their minimum and never fire.
- Modelling salvos and reloads from the installed cycle: rejected, since the
  composition with the engine's clock is unreadable.
- Applying the multiplier as the unit loses health on read: rejected, because
  the impact is where the damage type is known.
