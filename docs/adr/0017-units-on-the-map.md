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

- **The replay run as its own subcommand, and the order of its steps (REN-032,
  REN-057, REN-058).** `supercom replay <file> [--speed <x>] [--frames <n>]`
  plays a recorded match; `view` is unchanged. The run is the display first, as
  `view` has it, then the install, the replay file and its parse, the map folder
  the header names, the cache directory, the world, the content check and the
  start state, the map file and its parse, and then the window. Ground: each
  step needs the one before it; the cheap checks go first, and the display
  before any file is read keeps a headless host's failure free of everything
  else, as for `view`. The header's map folder is held to REN-032's
  single-component rule because the header is read from a file that a person
  supplied, and an unchecked folder name would walk out of the install's `maps`
  directory. A replay with no checkpoint is refused, since the stop rule would
  give it no end. Every step before the window is one function whose failures
  are error values naming the step, so a headless test reaches all of them but
  the last two.
- **The cache directory's rule (REN-059).** The importer's cache is
  `SUPERCOM_CACHE_DIR` itself, else `XDG_CACHE_HOME` joined with `supercom`,
  else the home directory joined with `.cache/supercom`; an empty value counts
  as unset, a relative result is refused, and an environment that names none is
  refused. Ground: a cache directory chosen from the working directory would
  write inside whatever repository the person ran the binary from, and a cache
  is state the person never asked to see; the rule is a pure function of three
  values so a test passes the values and never touches the real environment. The
  importer creates a missing directory, so the binary does not.
- **One owner of the state, and the comparison by the committed rule (REN-060,
  REN-061, REN-062, REN-063, REN-075, REN-076).** Exactly one module of the
  binary, the window's, holds a mutable state, with the kept values and the
  counters beside it; every other module reads the state through a shared
  reference. The comparison is a pure function of a tick, the digest of the step
  run at that tick, and the checkpoint list, under the rule the committed log is
  held to: the checkpoint at tick t is the t-th digest of the simulation's own
  replay run. The stop rule takes exactly the log's last checkpoint tick plus
  one steps. Ground: two oracles for the stepping, the committed rule and the
  simulation's own run, which the conformance test sets against each other; a
  second place that could step the state would be a second place for the two to
  diverge. A mismatch stops the run at once with the tick named, because every
  later digest chains from the wrong one.
- **Pacing that keeps held ticks due and counts the frames that held them
  (REN-064, REN-076).** The ticks owed are the whole ticks of the elapsed wall
  time times the speed times ten, less the ticks already run; a frame steps at
  most 10 of them, and the rest stay owed for later frames. The clock is never
  re-based, so a slow frame is never forgiven: the run's last line counts the
  frames at which ticks were held back. Ground: a pacing that dropped the ticks
  it could not step would hide a machine that cannot hold the tick behind a run
  that looks smooth, and holding the tick is the one timing fact the run
  reports. The cap bounds one frame's work to a second of match time at speed 1,
  so a stalled window cannot make the next frame step minutes of match time
  before drawing. The speed's bounds are 0.1 to 10,
  this project's choice: below 0.1 a tick lasts a second or more of wall
  time, and at 10 a frame owes a tick and two thirds at 60 frames a second,
  so the cap holds ticks back only on a frame slower than a tenth of a second.
- **Interpolation over one tick (REN-065).** A frame draws between the values
  before the last step taken in it and the values after it, by the fraction of a
  tick left over; headings take the short arc. An entity with no value from
  before is drawn where it is, and a frame that steps nothing keeps its values,
  so an entity that has just been created does not swing in from the origin.
  Ground: at 10 ticks a second, a marker that jumps once a tick looks like a
  stutter at 60 frames a second, and interpolating over one tick adds no latency
  beyond it.
- **Markers as flat instances, and what tells the four shapes apart (REN-066,
  REN-068, REN-071).** Every entity and projectile is one flat instance of one
  of four models: a square for a kind that does not move, a triangle for a
  mobile unit, a dart for a commander, and a narrow rhombus for a projectile,
  the triangle and the dart pointing along the model's +x so that a heading
  shows. Ground: the flat style is never hidden by terrain, which a marker
  standing for a unit must not be (REN-048); four silhouettes tell the classes
  apart with no colour, so a colour-blind person reads them too. A marker is at
  least 0.02 of the camera's distance across, so a unit one world unit across is
  not lost at the strategic zoom, where the distance is 309.02 on a square map
  256 units a side and a kind's own size is under 1 percent of the view.
- **The colours and the dimming (REN-067).** Army n takes entry n minus one of a
  table of eight linear colours and the table repeats; an unfinished entity is
  dimmed by 0.4 plus 0.6 times its build progress. Ground: the colours are
  chosen to stay apart on the terrain's ramp and on water, not to match the
  original's team colours, which arrive with the unit shading; dimming by
  progress shows a building going up at no cost, and the floor of 0.4 keeps a
  just-begun unit visible.
- **The drawn height from the terrain's surface, and why it departs from the
  simulation's y (REN-069, REN-070).** An entity on the land or the seabed is
  drawn at the height of the drawn terrain under it, which the renderer gives as
  a pure function of its mesh, using the mesh's own split of a cell; every other
  entity is drawn at the simulation's y. Ground: the simulation's y for a ground
  unit is the height of the cell it stands in, a step function of the position,
  while the drawn surface is the mesh's triangles; a marker at the cell's height
  would sink into a slope or hover over it by up to the slope's rise across a
  cell. The renderer holds the split, so the binary does not restate it. An
  entity outside the extent keeps the simulation's y.
- **The end of a run (REN-072, REN-073).** When the step at the last
  checkpoint's tick has run, stepping stops and the window stays open on the
  final state; the summary line gives the ticks run, the checkpoints matched out
  of how many, and the frames that held ticks back, and is written once, when
  stepping stops or, if the run ends sooner, when it ends. With `--frames <n>`
  the run ends after n frames, writes the summary, prints
  `presented <n> frames`, and succeeds, as `view` does. Ground: the line is
  the run's evidence for a person and for a script; writing it at the moment
  stepping stops puts it in front of a person watching the window.
- **One pixel test of the whole path (REN-074).** The start state of the
  committed log, the instance list at the strategic view, drawn offscreen, and
  the pixel of each commander's drawn position read back. Ground: every pure
  function of the unit is tested alone, and the frame is the one place their
  composition shows; two armies' colours at two projected points catch a swapped
  colour, a wrong model index, and a transform in the wrong space, none of which
  a test of one function sees.

- **The loader's labels follow a measurement, not the importer (FMT-060).** The
  importer FMT-027 cites names a vertex's three vectors tangent, normal,
  binormal at its line 281. A census of the install's 1,938 version-5 meshes
  found the second stored vector to be the surface normal in every one: it has
  the greatest mean absolute cosine with the face normal, and 1,839 meshes
  agree with it on a share of at least 0.99. `crates/scm` therefore reads the
  second stored vector into `normal` and the third into `tangent`, and says
  what was departed from. Ground: the draw takes its normals from this field,
  and a normal read from the third vector would light every mesh wrongly; the
  conformance test asserts the census's counts exactly, so a loader that
  followed the importer again fails it.
- **A mesh is drawn from its stored coordinates with one forward yaw (REN-077,
  REN-078, REN-079, REN-080, REN-081).** The only fix-up between a stored mesh
  and what is drawn is a yaw of a quarter turn at placement, which takes a
  mesh's +z to the unit's heading, with the blueprint's `Display.UniformScale`
  and no other change: no mirror, axis swap, winding reversal, rest transform,
  bone, `rest_pose_inverse`, tangent, or binormal. This is this project's
  choice. Of the five kinds in the committed log, all five have +y up and their
  triangles wound counter-clockwise from outside;
  the three with a Muzzle bone have it forward, at +z; and
  of the 50 bones counted for the rest pose, 48 have their vertices nearer the
  bone's place than the origin (19 of 19, 18 of 18, 9 of 11, and 2 of 2, none
  counted in urb1101). These are consistent with it, and the stored positions
  are drawn as the rest pose as this project's reading of them. No record states
  a side of a unit or a handedness.
- **The kinds of the start state and the build orders are resolved before the
  window (REN-082, REN-083, REN-084, REN-085).** The kinds are the blueprints of
  the start state's entities and of every build order, keyed in ASCII lower
  case. Each kind's mesh and scale are read through `supercom-scd`,
  `supercom-scm`, and `supercom-bp`, and a kind that fails to resolve ends the
  run with an error value naming the kind and the step. Ground: a mesh that
  fails to load must not appear as a blank unit minutes into a run, and the
  model set is uploaded once, so every kind must be known before the loop.
  Taking the kinds from the start state
  leaves the faction's name to the simulation, which alone parses it, and covers
  whatever the start state places.
- **The renderer refuses a normal shorter than 1e-6 (REN-086).** The meshes of
  UXB0008 and UXB0009 hold vertices whose normal is shorter than that, so a run
  whose kinds include either ends at the model check. Mending installed normals
  is not done here. Ground: a zero normal lights as black or as noise, and a
  refusal at the check is a value, where the shader would hide it.
- **Mesh or marker by one pure rule (REN-087, REN-088, REN-090).** A unit is
  drawn as its lit mesh when the camera is at most 50 times its own size away,
  and as its marker otherwise, never both. Ground: the marker's size floor is 2
  per cent of the camera's distance (REN-068), so beyond 50 times its size a
  unit is already drawn larger than itself; within 50 times its own size the
  floor does not bind, and the mesh takes over at the distance
  where the marker stops being enlarged. The close-zoom pixel test shows the
  mesh lit by the other frames it must not equal: flat, absent, recoloured.
- **Kinds met but not resolved are markers, and are counted (REN-089, REN-091,
  REN-072).** A unit of a kind the run did not resolve is drawn as its marker at
  every zoom, and the run's last line counts such kinds. The committed log names
  five kinds and meets no other, so its summary line ends
  `0 kinds drawn as markers`. The windowed test plays it at ten times the speed
  and expects exactly one summary line.
- **The census is this choice's known limit.** Of the 230 unit meshes with a
  Muzzle bone, the counts with a mean z above 0.5, below -0.5, and within 0.5 of
  zero are, by class, land 65, 4, 8; air 27, 6, 5; structure 50, 20, 12; sea 14,
  16, 3. Of the 78 unit meshes with both Left and Right bones, the Left bones'
  mean x is positive and the Right bones' negative in 30, the reverse in 45, and
  neither in 3, so the side names do not say which side is the unit's own; a
  bone's name is matched with regard to case, Muzzle anywhere in it, Left or
  Right at its start. These bone counts are measurements of 2026-11-17 that the
  suite does not re-run; it pins the bones only for the five kinds of the
  committed log, while FMT-060's test re-runs the census of the vectors over
  every mesh.

## Extensions of the first-light record

- **The near end of the zoom range is 8 world units where
  docs/adr/0016-first-light-renderer.md decided 40 (REN-010).** The head ruled
  on 2026-11-17 that the near end is 8, on the figure the head was shown: at a
  near end of 40, a commander's mesh is about 29 pixels tall in the default
  window. On SCMP_012 both start positions lie about 13 units under the plane of
  the map's highest vertex, where the target stays (REN-009). At a near end of
  40 and the near pitch of 35 degrees (REN-012), the
  line of sight through the screen's centre meets the ground at a start position
  63.4 units from the eye, where a length across the line of sight measures
  13.7 pixels per world unit in a window 720 pixels high; at a near end of 8
  that point is 31.4 units from the eye, at 27.7 pixels per world unit, and the
  commander's mesh is about 58 pixels tall. The other camera rules of REN-009 to
  REN-017 stay: the target stays on the plane of the map's highest vertex, so a
  camera that follows the ground is issue 84 of
  https://github.com/tgunnoe/SuperCom and not this record. The test of REN-017
  now takes an extent whose strategic view lies between 8 and 40 world units as
  valid, and REN-023's fixture is adapted so that the east and the west still
  differ at the nearer end.

- **REN-032's command line gains `replay`.** The first-light record's command
  line, `view` with a folder and `--frames`, is extended by `replay <file>` with
  `--speed <x>` and `--frames <n>`; every list accepted before parses as before,
  and every list rejected before still fails unless it begins with `replay`.
- **The binary now depends on the simulation's crates.** The first-light record
  says the binary depends on no loader or simulation crate (REN-034's ground).
  The replay run needs the simulation and the ingest that builds its world, so
  the binary's manifest gains `supercom-sim` and `supercom-sim-ingest` and
  nothing else; the rest of that record's ground for the map file's rule stands,
  since `view` still re-implements the lookup. Through `supercom-sim-ingest`
  the binary now also builds `supercom-scd`, `supercom-bp`,
  `supercom-bp-import`, `supercom-data`, and the Nickel evaluator
  `nickel-lang-core`. The binary never names the archive, mesh, or blueprint
  loaders in this unit, and never depends on the replay harness or the
  plugin host.
  The binary's manifest also gains supercom-scd, supercom-scm, and
  supercom-bp, for the meshes and their scales; the binary already builds the
  first and the third through supercom-sim-ingest, the mesh loader is new to
  its tree, and no package enters Cargo.lock.

## Consequences

- The renderer's public surface grows by one model type, one instance type,
  one style type, six pure functions, the model shader's source function,
  one constant for an instance's byte count, one method of the terrain mesh
  that gives the drawn height under a point, and two operations on the
  renderer; the draw function and the terrain upload are unchanged. A model's
  vertex and index byte counts are the terrain's own constants.
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
