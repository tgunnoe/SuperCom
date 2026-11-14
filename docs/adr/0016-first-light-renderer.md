# ADR-0016: The first-light renderer: a headless core of pure functions

**Status:** ACCEPTED (ratified by the head on 2026-11-14, as written, after running the window on SCMP_012 and SCMP_016; the section After ratification lists what was written or corrected since)

**Date:** 2026-11-11

## Context

The first slice of the renderer draws the installed terrain and water from
above, with a camera the player can pan and zoom. It is split so that
everything which needs no graphics device is a pure function in its own
crate, `supercom-render`, testable without a display (AGENTS.md I4): the
terrain grid built from a parsed map, the water plane, the camera, and the
shading function. The GPU pipeline that draws them into any texture view
lives in the same crate and is decided below (REN-019 to REN-029); the window,
the `view` subcommand of the `supercom` binary, calls the pipeline's one draw
function and is decided below (REN-032 to REN-042).
docs/specs/render.md holds the clauses; this record is the source each design
clause names.

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
  foreign types, keeps a math crate out of the dependency list, and keeps the
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
  depends on `supercom-scmap`, `wgpu`, `bytemuck`, and `pollster`, and names
  no windowing type in its API; its public items name types only from std,
  `supercom-scmap`, itself, and `wgpu`. The event loop and the window belong
  to the application crate, as ADR-0003 has it; they hand the renderer a
  surface target and call its draw function.
- **One draw path, the camera's matrix as given (REN-019).** The renderer is
  built for one `wgpu::TextureFormat` and draws a frame for a `&Camera` into
  any view of that format through one function; the offscreen tests and the
  window call the same code. It writes `Camera::view_projection()` into the
  uniform unchanged: the matrix's column-major layout is WGSL's `mat4x4<f32>`
  (REN-008), and a second projection or a transposition in the renderer would
  be a second place for the convention to go wrong.
- **The target format's rule and the transfer function (REN-019, REN-021).**
  The shader writes linear colour and the format's own encoding does the
  rest: an sRGB format encodes on write, a Unorm format stores the value as
  it is. Tests compare against the pure shading function's colour encoded the
  same way, within 2 in 255 per channel, which covers the 8-bit rounding and
  the GPU's arithmetic against the CPU's. A surface constructor picks an sRGB
  format when the surface offers one.
- **The clear colour (REN-020).** Linear (0.01, 0.01, 0.02): darker than the
  darkest terrain colour (the ramp's first stop times the ambient share) so a
  margin is told from terrain, and not the water's colour.
- **Water: opaque, unlit, depth-tested (REN-022).** One flat quad in linear
  (0.04, 0.22, 0.45), written without blending. Terrain below the surface is
  then hidden by the depth test and terrain above it shows, which is the whole
  visible rule of this slice; blending would show the terrain through the
  water and make the share of water in a frame depend on the terrain's colour.
  Its blue exceeds its red and green, which no terrain colour's does (every
  ramp stop has blue below red), so a test tells them apart by channel order.
- **Depth and culling (REN-019, REN-022).** A 32-bit float depth target,
  cleared to 1.0, compared with `Less`, matching clip depth 0 at the near
  plane (REN-008); back faces are culled with counter-clockwise front faces,
  which is the mesh's winding seen from above (REN-003). The drawing tests
  fail if the winding and the cull disagree, since the terrain would vanish.
- **Orientation is checked on the GPU (REN-023).** The camera's matrix puts
  north up and east right (REN-012), but a renderer could still flip a frame
  by its viewport, its winding, or the order in which a readback is read. A
  test therefore draws an asymmetric map at both ends of the zoom range and
  reads the frame with row 0 at the top, so the whole path from the matrix to
  the pixels is held to the convention, not the matrix alone.
- **Shading per fragment (REN-021).** The vertex stage passes the elevation
  and the normal; the fragment stage normalizes the elevation over the mesh's
  range, guards a zero range, renormalizes the normal, and applies the ramp
  and the light as `shade` does. Interpolating colours computed at the
  vertices would bend the ramp across its middle stop, so the pure function
  would not be the shader's specification.
- **The shader is assembled from the Rust constants (REN-021, REN-029).** A
  pure function formats the light direction, the ambient share, and the ramp
  stops, which the shading module makes public, into the WGSL text as `f32`
  literals that round-trip, so no shading number is typed in both languages
  and a changed constant changes both sides.
- **The fit check and the default limit (REN-024, REN-025).** A grid fits when
  its vertex buffer (24 bytes a vertex: position and normal) and its index
  buffer (4 bytes an index, six a cell) are at most the device's maximum
  buffer size. The function takes the cell counts and the limit as integers
  and is evaluated before a mesh is built and before any buffer exists,
  because wgpu reports an oversized buffer through a handler that panics by
  default. The device is requested with wgpu's default limits lowered to what
  the adapter supports, never above, so the maximum buffer size is at most
  256 MiB: no host accepts a larger grid, and a host whose adapter supports
  less refuses a grid sooner.
- **Errors are values (REN-024, REN-025, REN-026, REN-027).** A missing adapter, a refused
  device request, a refused surface, an oversized grid, an inconsistent mesh,
  a draw before an upload, a zero-sized target, and a validation or
  out-of-memory error captured by an error scope around resource creation and
  around each frame each return a variant of the crate's error. wgpu's own
  errors are neither `Clone` nor `PartialEq`, so their text is carried as a
  `String` and the error type stops being `Copy`. The device is requested with
  no optional feature and with limits the adapter supports, so the panics the
  request documents for those cannot fire; for that reason no test can make
  the request fail, and the refused-device path is specified and implemented
  but unexercised.
- **The GPU tests' name, flag, and skip rule (REN-028).** A test that needs
  an adapter is named `conformance_gpu_`; it does not read the install, except
  the two that also draw an installed map. With no adapter it prints
  `SKIP: <reason>` and passes; with `SUPERCOM_REQUIRE_GPU=1` it fails, so a CI
  host with a GPU can demand that the tests ran. The decision is a pure
  function of two booleans because the skip line cannot be reached on a host
  that has an adapter. A test that dumps frames for a human to look at writes
  them only when `SUPERCOM_RENDER_DUMP` names an absolute directory outside
  the repository, as binary PPM, so no image can enter the tree by a test run.
- **The command line, parsed first (REN-032).** `supercom` with no argument is
  the bare run it was; `supercom view [<map folder>] [--frames <n>]` opens the
  window. The arguments are parsed by hand, without a parsing crate, by a pure
  function of the argument list that runs before the display or the install
  is touched, so a usage error reads the same on any host. Ground: the
  grammar is two shapes, and a pure parser is testable without a window.
  The folder is one path component, so `view ../x` cannot leave the install's
  `maps` directory: it has no separator and is not `.` or `..`. The count is
  a positive 32-bit integer of ASCII digits, so no sign or fraction is read
  as one.
- **The display is checked first (REN-033).** `view` creates the event loop
  before it reads the install, and winit's creation fails when none of
  `WAYLAND_DISPLAY`, `WAYLAND_SOCKET`, and `DISPLAY` is set. The binary
  reports that failure in words that name the three variables and returns the
  failure code. Ground: a headless host is the common case of a failure, and
  the cheapest check goes first. The test is compiled on Linux alone, the one
  platform where a display is found only through those variables.
- **The map file rule (REN-034).** The map is the one file whose name ends
  `.scmap`, ignoring case, in the folder; none or several is an error naming
  the folder. Ground: an installed map is a folder of one `.scmap` beside
  three Lua files, and guessing among several would load the wrong map
  silently. The binary re-implements the small lookup with std and
  `supercom-scmap` alone, so it depends on no loader or simulation crate.
- **The run's order, and the fit check before the build (REN-035).** The
  display, the install, the map file, its parse, the window, the renderer for
  its surface, the fit check against that renderer's device, the mesh, the
  water, the camera, the upload, the loop. Ground: each step needs the one
  before it, and the fit check needs the device but not the mesh, so a map the
  device cannot hold is refused by REN-024's pure function before its
  buffers are built (AGENTS.md I6: an error value, never a panic in wgpu's
  default handler). The steps up to the window are one function of the
  install directory and the folder, so a test reaches them over temporary
  directories.
- **The input bindings and their constants (REN-036).** The wheel zooms one
  step a line, away from the user toward the near end, and shift selects the
  fast step of REN-010; a pixel-delta wheel (a touchpad) counts 40 pixels to
  the line so that it zooms fractionally; WASD and the arrow keys pan by 0.04
  of the distance per key event, so a held key's repeat moves the target in
  proportion to the view; Home goes to the strategic view; Escape closes.
  Keys are read by physical position, so the pan keys are where they are on
  every layout. The constants are choices of this project; the original's
  bindings were not read. Ground: only the camera is driven in this slice.
- **Non-finite input is ignored (REN-037).** `Camera::zoom` and `Camera::pan`
  already return early for a value that is not finite; the clause states it
  and a test pins it with two mutants of the source, because a NaN would
  otherwise reach the target and the matrices and make every later frame
  invalid.
- **Resizing and the zero-sized window (REN-038).** A pure function of the
  size and the configured size says whether to draw and whether to configure.
  A minimized window has a zero dimension, which wgpu documents as a panic
  when configured; so nothing is configured, the camera is unchanged, and
  nothing is drawn until the size is positive. A lost or outdated surface is
  configured again; a run of 30 frames that did not present ends the run, so
  a surface that never recovers fails instead of spinning.
- **The frame count option (REN-039).** `--frames <n>` presents n frames,
  prints `presented <n> frames`, and exits with success, so a script or a test
  can run the window to a known end. Ground: a window loop otherwise ends only
  when a person closes it, and a test must end by itself.
- **The surface's format as a pure function (REN-040).** Which of a surface's
  formats the renderer draws in is a function of the offered list alone: the
  first sRGB format, else the first, else an error. The constructor calls it,
  so the choice is tested without a surface.
- **The window's wgpu calls inside the renderer's error scopes (REN-041).**
  Configuring a surface, acquiring a texture, and presenting report errors to
  the device, whose default handler panics. The renderer therefore offers a
  public method that runs a caller's calls inside the validation and
  out-of-memory scopes it already uses and returns the first captured error as
  a value, and the window runs all three inside it. The binary names no wgpu
  line of its own: the renderer re-exports wgpu for the types.
- **A normal that is not a finite unit vector is refused (REN-042).** A slope
  whose square overflows gave a normal of length zero, finite but not a unit
  vector; the build now refuses a normal that is not finite and of unit
  length, with the existing error variant widened to say so, because a zero
  normal shades as black without a diagnostic.
- **The event loop belongs to the application crate (all of the above).** As
  ADR-0003 has it, the binary owns the window and the event loop and names
  winit; `supercom-render` still names no windowing type. The binary hands the
  renderer an `Arc` of the window as the surface target and winit's owned
  display handle, which needs no unsafe code.
- **The test that opens a window runs only when asked (REN-039).** It is named
  `conformance_window_` and runs only when `SUPERCOM_REQUIRE_DISPLAY=1` is set,
  inheriting the display it is given, so a plain test run and the gate open no
  window on anyone's screen; a host with a display runs it inside a nested
  compositor that shows nothing. Without the flag it prints a skip line and
  passes.

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
  and level of detail are deferred until measurement shows a need. One limit
  is already known: a whole-grid terrain cannot hold the three installed maps
  that are 4096 cells a side. Their vertex buffer, at the renderer's vertex
  stride of 24 bytes, is 402,849,816 bytes and their index buffer 402,653,184,
  both past the default 268,435,456-byte buffer limit; the renderer refuses
  such a grid with an error (REN-024, REN-025), and drawing it is tracked as
  issue 69 of https://github.com/tgunnoe/SuperCom.

## Alternatives rejected

- **Vertex colours instead of a per-fragment ramp.** One shading call per
  vertex is cheaper, but colours interpolate linearly between vertices and
  the ramp bends across its middle stop, so the pure function would not
  describe what is drawn.
- **Blended water.** Shows the terrain through the surface, but makes a
  pixel's colour a function of two layers and the water's share of a frame
  depend on the terrain under it.
- **Requesting the adapter's full limits.** Would let a host with a large
  adapter limit accept a grid that the default limit refuses; the lowered
  default caps all hosts at 256 MiB, so no host accepts what the default
  refuses, though a host with less refuses sooner.

- **A math crate (glam, nalgebra).** Adds a dependency whose types would
  surface in the public API or force conversions, for a handful of
  operations; the slice's needs are small and independently testable.
- **Chunked terrain or level of detail now.** No measurement says the whole
  grid is too slow, and the chunking scheme depends on the GPU side's
  measurements; building it first is speculative. The one limit known without
  measurement, the three installed 4096-cell maps whose buffers pass the
  default limit, is recorded under Consequences and tracked as issue 69.
- **A fixed-pitch camera.** A straight-down camera at every zoom loses the
  relief at close range; a tilted camera at the far end hides the map's far
  edge; the continuous pitch serves both ends.
- **Waiting for camera numbers observed from the running original.** The
  installed data holds none, an observation is slow and gated on the head's
  availability, and the slice can be built and replaced clause by clause once
  one exists.

## Known at ratification

The head ratified this record on 2026-11-14 after running `supercom view` in
a desktop session on an Intel Arc B580 over Vulkan: once on SCMP_012, four
times on SCMP_016, and once on SCMP_029, which ended with the fit check's
message. The head's verdict was 'the tests you asked for all check out'; it
does not say which of the actions the request listed were tried, or whether
the running original was beside the window. Four limits were named with the
request to ratify, and they stand:

- **The window loop is checked by a person's run.** No headless test reaches
  the loop. REN-038's sentences on reporting an error from setting the aspect
  ratio once, on a lost or outdated surface, on the frames that did not
  present, and on running out of memory, and REN-041's sentence that the
  window's calls never report to the device's default handler, have a person's
  run as their only evaluator, and the output of the run above holds none of
  their messages; the pure functions they call are tested.
- **A lost surface is configured again, not recovered.** wgpu documents a
  lost surface as needing a new one, and the renderer keeps no instance to
  make one from; such a surface ends the run after the 30 frames of REN-038.
- **The driver can lose a device.** On the Intel Arc B580 the driver now and
  then loses a device that was just requested. It has been seen in the GPU
  tests and never on the software driver (issue 74 of
  https://github.com/tgunnoe/SuperCom). A failed device request fails a
  `conformance_gpu_` test with or without `SUPERCOM_REQUIRE_GPU=1`, since
  REN-028's skip covers a missing adapter only. Whether a lost device should
  skip when the flag is unset is decided with issue 74.
- **A lost device is named by nothing.** Neither the renderer nor the tests
  register a device-lost callback, and wgpu keeps a lost device's errors away
  from the error scopes. In `view`, a device lost after the window is built
  shows nothing new, and the run goes on or ends after the 30 frames with a
  message that does not mention the device (issue 74).

## After ratification

Written at the campaign's close on 2026-11-14, after the head's ratification;
the head had not seen it when ratifying.

- **Why a refused device request is not a skip.** A request that an existing
  adapter refuses may be the renderer's own fault: the renderer asks for no
  feature and for the default limits lowered to the adapter's own (REN-025),
  and a change that asked for more would show only as a refused request,
  which a skip would hide on every host with an adapter.
- **Corrected after ratification:** the statements of REN-007 (a stored water
  elevation that is not finite gives no plane), REN-016 (a refused aspect
  ratio includes one at which the strategic view would be no farther than the
  near end), REN-017 (a relief that is not finite is refused, and the near
  end's boundary reads "no farther than"), REN-029 (the source is parsed and
  validated without an adapter, and the uniform struct's layout is checked),
  REN-032 (an argument beginning with `-` is an option), REN-037 (the claim
  about two mutants is dropped), and REN-039 (the flag with no install found
  fails) now say what the code already did or what a test now checks. The
  bullet on hand-written math said "keeps the dependency list at
  `supercom-scmap`", true when it was written; the crate also depends on wgpu,
  bytemuck, and pollster, as the later bullet says, so the sentence now says
  only that no math crate is in the list.
- **Found after ratification:** the forge's runner has no graphics adapter, so
  every `conformance_gpu_` test skips there, and REN-029's parse and
  validation are the only check the shader gets there (issue 77 of
  https://github.com/tgunnoe/SuperCom).
- **Found after ratification:** REN-035's order (the fit check before the
  mesh) and REN-040's last sentence (the surface constructor uses the format
  choice) are reached by no test, and rest on reading and a person's run
  (issue 78 of https://github.com/tgunnoe/SuperCom).
- **Found after ratification:** a camera refused a new aspect ratio keeps its
  previous one, so the window draws stretched until a size is accepted; no
  installed map is thin enough for this.
