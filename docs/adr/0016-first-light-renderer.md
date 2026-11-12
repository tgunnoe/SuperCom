# ADR-0016: The first-light renderer: a headless core of pure functions

**Status:** PROPOSED (awaiting the head's ratification)

**Date:** 2026-11-11

## Context

The first slice of the renderer draws the installed terrain and water from
above, with a camera the player can pan and zoom. It is split so that
everything which needs no graphics device is a pure function in its own
crate, `supercom-render`, testable without a display (AGENTS.md I4): the
terrain grid built from a parsed map, the water plane, the camera, and the
shading function. The graphics device and the window are the next two units
of work and import what this crate makes public. docs/specs/render.md holds
the clauses; this record is the source each design clause names.

The installed game does not supply the camera. A search of its Lua (17
archives, 2,091 `.lua` members) finds no number for a world camera's field of
view or clip planes (no hit for NearZ, FarZ, FieldOfView, SetFOV, or the word
FOV) and finds the zoom limits only as engine calls (GetMaxZoom and
GetMinZoom, 4 hits each in 2 members), never as numbers. The field of view,
the zoom range, the pitch, the pan limits, the clip planes, and the shading
are therefore this project's choices, made here and stated as design clauses
that never describe the original. The vertical scale is the exception: a
reference reader multiplies the raw sample by the stored scale (REN-001), a
reading this record adopts and leaves open to comparison with the running
game.

## Decision

- **Coordinates (REN-008).** x along heightmap columns, y up, z along rows,
  right-handed, world units; matrices column-major as WGSL's `mat4x4<f32>`
  lays them out, so the GPU side uploads the array as it stands; clip space
  with y up and depth 0 at the near plane and 1 at the far plane, the
  convention wgpu uses. Ground: the heightmap is stored row by row, so z
  follows rows without a flip, and one convention shared with the GPU side
  prevents a transposed or flipped scene.
- **One vertex per sample (REN-002), 32-bit indices over the whole grid, an
  error past 32 bits (REN-003, REN-004, REN-005).** The largest installed
  maps are 4096 cells a side, 16,785,409 vertices, which 32-bit indices
  address; the pure bound function decides the limit from the cell counts
  without building a grid. Malformed input (a sample count that is not
  (width + 1) * (height + 1), zero cells, a non-finite scale or extent or
  elevation) returns an error, because the map loader hands over whatever a
  file claims and the library never panics (AGENTS.md I6). The cell is split
  along its north-east to south-west diagonal and wound counter-clockwise
  seen from above so a flat map faces up; the split is arbitrary and pinned
  so the GPU side and the tests agree.
- **Normals from neighbour differences (REN-006).** Central differences
  across a vertex, one-sided at an edge, are exact on a plane, which makes a
  property test possible (a tilted plane has a known normal), and cost one
  pass over the grid.
- **One water quad (REN-007).** The installed map stores one surface
  elevation; a flat quad at that elevation over the header's extent is the
  whole water of this slice. Water effects are a later slice.
- **Hand-written f32 math, no math crate (REN-008, REN-009).** The slice
  needs a look-at, a perspective, a product, and a handful of vector
  operations. Writing them as plain arrays keeps the public API free of
  foreign types, keeps the dependency list at `supercom-scmap`, and keeps the
  matrices testable against independent arithmetic written in the tests.
- **Camera model (REN-009, REN-010, REN-011, REN-012).** A vertical field of
  view of 45 degrees: a conventional middle value that frames a map from
  above without strong perspective. The target always lies on the plane of
  the map's highest vertex, so the eye (above that plane at every zoom) is
  higher than every vertex and no terrain can stand between the eye and its
  target. The zoom range is a distance interval serving HUD-011 of
  docs/specs/ui.md. Its far end is defined by the strategic view, the
  smallest distance at which the whole extent fits for the aspect ratio, so
  the HUD's strategic zoom always has its picture; its near end is 40 world
  units, close enough to read a few cells and well above the eye height the
  clip planes need. The pitch falls linearly with the distance from straight
  down at the far end to 35 degrees at the near end: the far end must look
  straight down for a map overview, and a tilt toward the near end gives
  depth to the relief. The view is built from the pitch directly (right is
  +x, up is derived), not from a look-at with an up hint, because a look-at
  is undefined when looking straight down. A zoom step moves the distance
  by 0.02 of the range at normal speed and 0.08 at fast speed, linear in
  distance and continuous; these fractions equal the ones the installed key
  actions use (lua/keymap/keyactions.lua of lua.scd, lines 176 to 182), the
  one readable fact about the original's zoom, adopted so that a held key
  feels as it does there. A fractional step zooms by that fraction, so a
  wheel or a held key can be continuous.
- **Pan clamp narrows with the distance (REN-013).** The allowed target
  region is the extent scaled about the centre by the zoom's fraction from
  the far end: the whole extent at the near end and the centre alone at the
  far end. Ground: with it, zooming out re-clamps the target toward the
  centre, so the far end shows the whole extent from any target (REN-011)
  without a second rule, and it is a single comparison per pan and per zoom.
  The other candidate, a fixed clamp with a target that drifts to the centre
  while zooming out, needs state beyond the camera's own fields.
- **Clip planes follow the distance (REN-014).** The near plane is half the
  smallest depth at which a point on or below the target's plane can lie in
  the view pyramid, from the eye's height above that plane and the
  pyramid's steepest ray; the far plane is 1.1 times a bound on the distance
  to any point of the extent between the lowest and highest elevations.
  Ground: both come from the geometry of the clause, with a margin for f32
  rounding, so the test of REN-014 can check them against its own pyramid
  computation. A fixed near and far plane would waste depth precision at the
  near end or clip the horizon at the far end.
- **Aspect changes and bad input (REN-016, REN-017).** The zoom range depends
  on the aspect ratio, so setting it recomputes the range and re-clamps. A
  camera refuses an extent so small that its strategic view is nearer than the
  near end, rather than inventing an empty range.
- **Shading (REN-015).** The height normalizes over the mesh's own range, so a
  consumer holding a vertex's y computes the same value. The colour is a
  three-stop linear ramp over the normalized height (0.18, 0.30, 0.12 at 0;
  0.50, 0.42, 0.28 at 0.5; 0.92, 0.90, 0.86 at 1) times a light factor of
  0.3 + 0.7 * max(0, n . l) for the unit normal n and the light direction
  l = (-0.36, 0.8, -0.48), a unit vector from the north-west and above. The
  function depends on height and slope alone, which is all this slice has;
  textures and shadows replace it later.
- **One crate, no windowing dependency (all of the above).** `supercom-render`
  depends on `supercom-scmap` alone and names no foreign type in its API. The
  event loop and the device belong to the application crate, as ADR-0003
  has it; the GPU crate imports this one.
- **Tolerance.** The inside-the-view comparisons use a relative tolerance of
  1e-4 of w: f32 arithmetic over a matrix product errs by about 1e-6
  relative, and at the far end the corners lie on the screen's edge by
  construction.

## Consequences

- The GPU side and the window import a stable, tested seam; a change to the
  convention or a constant is a change to a clause here.
- Every camera constant is a choice of this project and the head may
  overrule it at ratification; the clauses carry the numbers so a change is a
  one-place edit with its test.
- The pitch, field of view, and zoom fractions are not evidence about the
  original. REN-001's vertical reading, and how the result looks, are decided
  by comparison with the running game, which this record does not close.
- Whole-grid meshes of the largest maps hold 16.8 million vertices; chunking
  and level of detail are deferred until measurement shows a need.

## Alternatives rejected

- **A math crate (glam, nalgebra).** Adds a dependency whose types would
  surface in the public API or force conversions, for a handful of
  operations; the slice's needs are small and independently testable.
- **Chunked terrain or level of detail now.** No measurement says the whole
  grid is too slow, and the chunking scheme depends on the GPU side's
  measurements; building it first is speculative.
- **A fixed-pitch camera.** A straight-down camera at every zoom loses the
  relief at close range; a tilted camera at the far end hides the map's far
  edge; the continuous pitch serves both ends.
- **Waiting for camera numbers observed from the running original.** The
  installed data holds none, an observation is slow and gated on the head's
  availability, and the slice can be built and replaced clause by clause once
  one exists.
