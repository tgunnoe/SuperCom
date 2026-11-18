# ADR-0017: Units on the map: models and instances in the renderer

**Status:** PROPOSED (awaiting the head's ratification)

**Date:** 2026-11-17

## Context

The first-light renderer (docs/adr/0016-first-light-renderer.md) draws the
terrain and the water. A skirmish also needs things standing on the map, and
the renderer must draw them before anything in the binary decides what they
are. This record adds the renderer's half: it draws models at transforms in
colours, and learns nothing of what a model depicts. What the binary draws
with them, and the conversion from a match's state to instances, arrive with
their own code and record.

The renderer's public items name no concept of the simulation: no unit, army,
marker, blueprint, kind, tick, or replay, and the crate's manifest, source,
and tests name no crate of the simulation or of the loaders. A model is plain
positions, normals, and indices; an instance is a model's index, a transform,
a colour, and a style. docs/specs/render.md holds the clauses (REN-043 to
REN-056); this record is the source each names.

## Decision

- **Models and instances as plain data (REN-043, REN-046).** A model is
  positions and normals of equal count and 32-bit triangle indices, in the
  vertex layout the terrain already uses, so the vertex layout, the 32-bit
  indices, and the counter-clockwise front face stay one convention. An
  instance is a model index into the uploaded set, a column-major transform in
  the convention of REN-008, a linear colour, and a style. Textures,
  texture coordinates, animation, and level of detail are left out: this unit
  of work needs a lit or a flat solid at a place, and each later need
  extends the data it needs. Keeping the renderer ignorant of the
  simulation keeps the headless crate buildable and testable alone (AGENTS.md
  I4) and keeps the replay and determinism domain out of it.
- **What an upload and a list refuse, before any buffer exists (REN-043,
  REN-044, REN-045, REN-046, REN-047).** An upload refuses a model with no
  vertex or index, unequal position and normal counts, an index count that is
  not a multiple of three, an index at or past the vertex count, or a
  position or normal that is not finite; a list refuses an instance naming no
  model of the set, a transform that is not finite, and a colour channel that
  is not finite or outside 0 to 1. Each is its own error value, decided by a
  pure function, so no call that wgpu validates lazily is made with an
  argument the renderer has not checked and no well-formed call reaches wgpu's
  default error handler, which panics, as REN-025 requires for the terrain.
  The byte counts of the fit (24 bytes per vertex, 4 per index, 76 per
  instance) are stated in the clause, so the test pins them. A refused upload
  leaves the set and the list as they were; a good one replaces the set and
  empties the list, because an instance's model index names a place in the set
  it was made against.
- **The two styles and the draw order (REN-048).** A lit instance is drawn
  with the depth test Less and depth writes, back faces culled and
  counter-clockwise faces in front, as the terrain is; a flat instance is
  drawn after it with no depth test, no depth write, and no culling, in the
  colour as given. The one draw function keeps its signature and stays the
  only draw path (REN-019); the instances follow the terrain and the water in
  its one pass. A flat instance serves what must never be hidden, such as a
  marker behind a hill; a lit one serves bodies that terrain can stand in
  front of. No blending and no shadows are used.
- **The placement transform and the sign of its turn (REN-050, REN-051).**
  Headings are measured as the simulation measures them, 0 along +x and
  increasing toward +z. With +z south and y up, that is the opposite of the
  usual positive turn about +y, so the sign is pinned by literals in the test
  and the clause states the turn as one angle: every horizontal direction a
  goes to a + h - f for heading h and the model's forward yaw f. The function
  returns an error value for a scale that is not finite and positive and for
  an angle or a coordinate that is not finite, so that no transform it returns
  reverses a triangle's winding.
- **Lit shading from the terrain's light (REN-052, REN-053).** A lit
  instance's colour is the colour times the ambient share plus the light's
  share, with the terrain's own light direction and ambient share, so a unit
  and the ground under it are lit alike. The normals are turned by the
  upper-left three by three part of the transform and renormalized, which is
  exact for the transforms of REN-050 (a rotation and a uniform scale); the
  shader renormalizes after interpolation as the terrain's does. A flat
  instance's pixel is its colour.
- **The model shader as a second module (REN-054).** The lit and flat
  pipelines use a second WGSL module, assembled from the same Rust constants
  by its own pure function, with the first module left byte-identical so
  REN-021 and REN-029 stay true as written. The two modules share the
  uniform's layout and read the camera's view-projection matrix as given.
- **The unchanged picture (REN-049).** A frame with no model set, and a frame
  with a set and an empty list, are equal pixel for pixel: the pass sets no
  state that changes what the terrain or the water draws, and the test
  compares frames, after first requiring that a one-instance list changes one.
- **The terrain and the model set are independent (REN-056).** The terrain
  upload leaves the set and the list as they were, and either may come first;
  a draw with no terrain stays REN-027's error. A replacement of the terrain,
  such as a new map, therefore does not silently drop what the caller set.
- **Direction on the screen (REN-055).** At the strategic view a model's own
  +x lies to the right of the frame at heading 0 and toward its bottom at
  heading pi/2, which ties the placement's sign to what a person sees.

## Extensions of the first-light record

- **The near end of the zoom range is 8 world units where
  docs/adr/0016-first-light-renderer.md decided 40 (REN-010).** The head ruled
  on 2026-11-17 that the near end is 8, on the figure the head was shown: at a
  near end of 40, a commander's mesh is about 29 pixels tall in the default
  window. On SCMP_012 both start positions lie about 13 units under the plane
  of the map's highest vertex, where the target stays (REN-009). At a near end
  of 40 and the near pitch of 35 degrees (REN-012), the line of sight through
  the screen's centre meets the ground at a start position 63.4 units from the
  eye, where a length across the line of sight measures 13.7 pixels per world
  unit in a window 720 pixels high; at a near end of 8 that point is 31.4
  units from the eye, at 27.7 pixels per world unit, and the commander's mesh
  is about 58 pixels tall. The other camera rules of REN-009 to REN-017 stay:
  the target stays on the plane of the map's highest vertex, so a camera that
  follows the ground is issue 84 of https://github.com/tgunnoe/SuperCom and
  not this record. The test of REN-017 now takes an extent whose strategic
  view lies between 8 and 40 world units as valid, and REN-023's fixture is
  adapted so that the east and the west still differ at the nearer end.

## Consequences

- The renderer's public surface grows by one model type, one instance type,
  one style type, six pure functions, the model shader's source function,
  three constants for the byte counts, and two operations on the renderer; the draw function and the terrain upload
  are unchanged.
- The binary can place anything it can describe as a model, in a colour,
  without the renderer changing. Models from the installed game's meshes, the
  team colours, and what the binary draws arrive with their own records.
- Every instance of a list is submitted: there is no visibility culling, no
  instancing by anything but a model's run in the list, and no
  frame-time work beyond the draw. Measurement decides any of these later.
- A frame costs one draw call per run of consecutive instances that share a
  model and a style, after the terrain's and the water's.
- The nearer end of the zoom range puts the eye 8 world units from its
  target at a pitch of 35 degrees, so a camera that follows the ground
  (issue 84) is the next camera work, and over high ground the eye is closer to
  terrain that stands above the target's plane.

## Alternatives rejected

- **A renderer that knows units.** It would pull the simulation's types into
  the headless crate, tie the renderer's tests to the simulation, and put the
  mapping from a match's state to pixels where it cannot be replaced; plain
  models and instances keep that mapping in the caller.
- **One style.** Lit alone hides a marker behind terrain; flat alone gives a
  body no form. Two styles cost two pipelines and one draw order.
- **Blended or sorted flat instances.** Blending makes a pixel a function of
  draw order and of two layers; the flat style states its colour as given
  and relies on no ordering among flat instances.
- **Editing the first shader module.** One module for both would change the
  text REN-021 and REN-029 describe and risk the terrain's picture; a second
  module keeps the first byte-identical.
- **A transform built by the shader from position, heading, and scale.** It
  would put the sign of the turn in a language the renderer's headless tests
  cannot run; the pure function is tested without a device and the shader
  reads a matrix.
- **Camera that follows the ground now.** It changes REN-009's rule for the
  target and its clauses, and is tracked as issue 84 of
  https://github.com/tgunnoe/SuperCom.
