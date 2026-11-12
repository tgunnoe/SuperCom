# ADR-0016: The first-light renderer: a headless core of pure functions

**Status:** PROPOSED (awaiting the head's ratification)

**Date:** 2026-11-11

## Context

The first slice of the renderer draws the installed terrain and water from
above, with a camera the player can pan and zoom. It is split so that
everything which needs no graphics device is a pure function in its own
crate, `supercom-render`, testable without a display (AGENTS.md I4): the
terrain grid built from a parsed map, the water plane, the camera, and the
shading function. The GPU pipeline that draws them into any texture view
lives in the same crate and is decided below (REN-019 to REN-029); the window
is the next unit of work and calls the pipeline's one draw function.
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
  the adapter supports, never above, so the maximum buffer size is 256 MiB on
  every host and the answer does not depend on which adapter was found.
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
- **Requesting the adapter's full limits.** Would let the fit check pass a
  grid on one host that another refuses; the default limit makes the answer
  the same everywhere.

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
