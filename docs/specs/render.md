# Render specification

This file specifies the first slice of the renderer. Its first part needs no
graphics device: the terrain grid built from a parsed map, the water plane,
the camera with its zoom range and clip planes, the height normalization,
and the shading function, all pure functions over plain `f32` arrays in the
crate `supercom-render`. Its second part is the GPU pipeline in the same
crate, which draws that terrain and water for a camera into any texture view
through wgpu; it needs an adapter but no window, and its tests draw
offscreen. Its third part is the windowed mode, the `view` subcommand of the
`supercom` binary, which opens a window on an installed map and calls the
pipeline's one draw function; its decisions are pure functions of the
binary, tested without a window. Its camera and shading are decisions of this project, recorded
in docs/adr/0016-first-light-renderer.md: a search of the installed data finds
no number for a world camera's field of view, clip planes, or zoom limits, so
no clause below says how the original behaves.

The frontier of this slice: the GPU pipeline's and the windowed mode's
contracts are specified below; the window loop itself, the code that turns the
window system's events into inputs and draws, is reached by no headless test
and is tried only when a display is available, so a person's run is its
evaluator (the record's last section says what the run at its ratification
covered); a graphics device lost by the driver is reported by nothing (issue
74 of https://github.com/tgunnoe/SuperCom); the three installed maps of 4096
cells a side are refused and not drawn (issue 69); terrain splatting, decals,
shadows, water effects, units, fog of war, strategic icons, and
post-processing are later slices; and the vertical reading of REN-001 awaits
a comparison with the running original, the first look at the result, on
SCMP_012 and SCMP_016, having raised no mismatch.

## Coordinates and conventions

**REN-008** `design` `test:render_matrices_are_column_major_with_depth_zero_to_one`
The renderer's coordinate convention MUST be: x along heightmap columns (east), y up, z along heightmap rows (south), right-handed, in world units; matrices column-major, so `m[c][r]` is row `r` of column `c` and a point maps to clip space as the matrix times the column vector `(x, y, z, 1)`; clip space with y up and depth 0 at the near plane and 1 at the far plane. The test projects points on the view axis at the near and far clip distances and expects depths 0 and 1 and a w equal to the distance (docs/adr/0016-first-light-renderer.md).

## Terrain

**REN-001** `reference` `test:render_elevation_is_sample_times_scale`
A vertex's elevation MUST be its raw heightmap sample, read as a signed 16-bit value, multiplied by the heightmap's stored scale. The pinned reference (https://raw.githubusercontent.com/FAForever/Neroxis-Map-Generator/10b130836034/shared/src/main/java/com/faforever/neroxis/map/SCMap.java) declares the scale field with its default of 1/128 at line 72 (`private float heightMapScale = 1f / 128f;`) and, at line 649 inside `setObjectHeights` (declared at line 641), multiplies the raster sample by that scale to set an object's world height; it applies this computation when it places objects and never computes a terrain surface. This project applies the same computation to the terrain's vertices as its reading, and that reading's match with the original's relief stays open until it is compared with the running game. On the 60 installed maps no sample is negative and the largest is 9755.

**REN-002** `design` `test:render_grid_has_one_vertex_per_sample`
The terrain grid MUST have one vertex per heightmap sample, (width + 1) by (height + 1) for a map of width by height cells, the vertex of column `i` and row `j` at x = `i` times (map width / cell columns) and z = `j` times (map height / cell rows), so the grid spans exactly the header's extent (docs/adr/0016-first-light-renderer.md).

**REN-003** `design` `test:render_indices_form_two_upward_triangles_per_cell`
The terrain indices MUST be 32-bit, two triangles per cell, 6 times width times height of them, every index below the vertex count, each cell split along the diagonal from its north-east to its south-west vertex, wound so that every triangle of a flat map faces up (docs/adr/0016-first-light-renderer.md).

**REN-004** `design` `test:render_malformed_maps_are_errors`
Building the terrain from a map whose sample count is not (width + 1) * (height + 1), whose cell counts are zero, whose scale is not finite, whose extent is not finite and positive, or any of whose elevations is not finite MUST return an error value and MUST NOT panic (docs/adr/0016-first-light-renderer.md).

**REN-005** `design` `test:render_vertex_count_bound_is_32_bits`
A grid whose vertex count, (columns + 1) * (rows + 1) computed without overflow, does not fit 32 bits MUST be an error; the bound is decided by a pure function of the two cell counts and is tested through that function, never by building such a map (docs/adr/0016-first-light-renderer.md).

**REN-006** `design` `test:render_normals_are_unit_length_and_up_on_flat_terrain`
Every vertex normal MUST be unit length and MUST be straight up on flat terrain; a normal is computed from the height differences of the vertex's neighbours along x and z, taken across the vertex where both neighbours exist and from the vertex to its one neighbour at an edge, so it is exact on a plane (docs/adr/0016-first-light-renderer.md).

**REN-007** `design` `test:render_water_plane_matches_stored_elevation`
The water plane MUST be one quad, facing up, at the stored surface elevation, spanning the header's extent, when the map carries water, and absent when it carries none (docs/adr/0016-first-light-renderer.md).

## Camera

**REN-009** `design` `test:render_target_projects_to_centre_on_highest_plane`
The camera's view-projection MUST project the camera's target to the centre of the screen; the target MUST lie on the horizontal plane at the map's highest vertex elevation, and the eye MUST never be below that plane. The vertical field of view is 45 degrees, a choice of this project (docs/adr/0016-first-light-renderer.md).

**REN-010** `design` `test:render_zoom_range_far_end_is_the_strategic_view`
The zoom range MUST be a distance interval, serving the continuous tactical-to-strategic zoom HUD-011 of docs/specs/ui.md requires. Its far end MUST be the smallest distance at which, looking straight down with the target at the map's centre, the whole map extent on the target's plane is inside the view for the given aspect ratio (the strategic view); inside the view means clip-space x and y no larger than w in magnitude, compared with a relative tolerance of 1e-4, because at the far end the extent's corners lie on the screen's edge by construction. Its near end MUST be 40 world units. Zooming MUST clamp to the range, the distance MUST be continuous within it, and one step MUST move the distance by 0.02 of the range at normal speed and 0.08 at fast speed, a positive number of steps zooming toward the strategic end and a fractional number zooming by that fraction of a step (docs/adr/0016-first-light-renderer.md).

**REN-011** `design` `test:render_zoom_out_from_a_corner_shows_the_whole_extent`
At the far end of the zoom range the whole extent MUST be in view, whatever the target was before zooming out, so HUD-011 of docs/specs/ui.md always has its strategic view. The target's clamp narrows with the distance (REN-013) and is the map's centre at the far end, so zooming out moves a target toward the centre. The test zooms out by steps from a target at each corner to the far end and expects the extent's four corners, at the highest vertex elevation, to project inside the screen, and expects that at a distance one percent smaller, with the target at the centre, at least one corner does not (docs/adr/0016-first-light-renderer.md).

**REN-012** `design` `test:render_pitch_tilts_continuously_with_north_up_east_right`
The camera MUST look straight down at the far end of the zoom range and tilt toward a pitch of 35 degrees at the near end, the pitch (the angle of the view direction below the horizontal, 90 degrees when straight down) falling linearly with the distance from 90 degrees at the far end to 35 degrees at the near end. The view matrix MUST be finite at every zoom, the straight-down end included, with north (decreasing z) up on screen and east to the right: at the far end and at the near end, a point east of the target has a larger clip-space x than the target, and a point north of it a larger clip-space y (docs/adr/0016-first-light-renderer.md).

**REN-013** `design` `test:render_pan_clamps_target_inside_narrowed_extent`
Panning MUST move the target in x and z and clamp it to the map's extent as narrowed by the zoom: the allowed target region is the extent scaled about the map's centre by (far - distance) / (far - near) of the zoom range, the whole extent at the near end and the centre alone at the far end (docs/adr/0016-first-light-renderer.md).

**REN-014** `design` `test:render_clip_planes_bound_every_visible_vertex`
The near and far clip planes MUST be functions of the distance such that, at every allowed zoom and target, every vertex of the map that lies inside the view pyramid (inside the frustum's four side planes) has a depth between the near and far planes, and the eye is higher than the map's highest vertex. The near plane is half the smallest depth at which a point on the target's plane or below can lie inside the pyramid, and the far plane is 1.1 times a bound on the distance from the eye to any point of the extent between the map's lowest and highest elevations: the hypotenuse of the extent's diagonal plus the eye's horizontal offset from the target and of the eye's height above the target's plane plus the relief (docs/adr/0016-first-light-renderer.md).

**REN-016** `design` `test:render_aspect_change_recomputes_range_and_reclamps`
Setting the aspect ratio MUST recompute the zoom range and re-clamp the distance and the target; an aspect ratio that is not finite and positive MUST be refused and MUST leave the camera unchanged (docs/adr/0016-first-light-renderer.md).

**REN-017** `design` `test:render_camera_rejects_bad_inputs`
Constructing a camera MUST return an error value, never panic, for an extent or an aspect ratio that is not finite and positive, an elevation that is not finite, a highest elevation below the lowest, or an extent so small that its strategic view is nearer than the near end of the zoom range; a flat map, highest equal to lowest, is valid (docs/adr/0016-first-light-renderer.md).

## Shading

**REN-015** `design` `test:render_height_normalization_and_shading`
The height normalization MUST map an elevation to (elevation - lowest) / (highest - lowest) over the mesh's own elevation range, and a range of zero width MUST normalize to 0 without a NaN. The shading function MUST map a normalized height and a unit normal to a linear RGB colour with every channel in 0 to 1, brighter as the normal turns toward the light direction (-0.36, 0.8, -0.48), the unit vector from the surface toward the light (docs/adr/0016-first-light-renderer.md).

## The installed maps

**REN-018** `game-data` `test:conformance_render_installed_maps`
On the installed map SCMP_012 the terrain grid MUST have 66,049 vertices and 393,216 indices, its lowest and highest vertex elevations MUST be 3.453125 and 20.1640625 (the samples 442 and 2581 times the stored scale 0.0078125), and it MUST have no water plane; on the installed map SCMP_016 the water plane MUST sit at elevation 17.5.

## The GPU pipeline

The renderer draws into a texture view of one format it was built for. A
`conformance_gpu_` test depends on an adapter and not on the install, except
the two that read the installed maps and call `require_game` first.

**REN-019** `design` `test:conformance_gpu_one_draw_function_fills_views_of_any_size`
A renderer MUST be built for one target texture format and MUST draw a frame for a camera into any texture view of that format, of any positive size, through one function that is the only draw path, so the offscreen tests and the windowed mode run the same code; it MUST write the camera's view-projection matrix (REN-008) into the shader's uniform as given, without transposing it or projecting by itself. The test draws one scene into two offscreen views of different sizes through that function and expects terrain pixels in both (docs/adr/0016-first-light-renderer.md).

**REN-020** `design` `test:conformance_gpu_empty_margins_hold_the_clear_colour`
A pixel that no geometry covers MUST hold the clear colour, linear RGB (0.01, 0.01, 0.02), encoded with the target format's transfer function. The test draws the square map of the east plane into a view twice as wide as it is tall, expects the centre to be terrain, and only then expects the corners and the side margins to be the clear colour within 2 in 255 per channel (docs/adr/0016-first-light-renderer.md).

**REN-021** `design` `test:conformance_gpu_terrain_pixels_match_the_shading_function`
A terrain pixel MUST hold the colour of the shading function (REN-015) for that fragment: the elevation interpolated to the fragment normalized over the mesh's own range by the normalization of REN-015, a range of zero width included, and the interpolated normal renormalized; not an interpolation of colours computed at the vertices, which cannot follow the ramp across its middle stop. The light direction, the ambient share, and the three ramp stops in the shader source MUST be written from the crate's Rust constants. The colour MUST be encoded with the target format's transfer function, the sRGB encoding for an sRGB format and the identity for a Unorm format, and MUST be within 2 in 255 per channel of that. The test draws four synthetic planes (rising east, rising south, rising diagonally, flat) in Rgba8UnormSrgb, Bgra8UnormSrgb, and Rgba8Unorm, finds for each sampled pixel the surface point its centre sees, and compares the pixel with the colour the pure function gives there, over normalized heights from below 0.2 to above 0.8 and four distinct normals (docs/adr/0016-first-light-renderer.md).

**REN-022** `design` `test:conformance_gpu_water_hides_only_terrain_below_its_surface`
The water plane MUST be drawn at its stored elevation, opaque, in the linear RGB colour (0.04, 0.22, 0.45) with no lighting, with depth testing against the terrain, so terrain below the surface shows the water's colour and terrain above it shows its own. The water's colour MUST be told from every terrain colour by its blue channel exceeding both its red and its green. The test draws a plane that crosses the surface and samples both sides (docs/adr/0016-first-light-renderer.md).

**REN-023** `design` `test:conformance_gpu_north_is_up_and_east_is_right_at_both_zoom_ends`
In a frame read back with row 0 at the top, north (decreasing z) MUST be at the top and east (increasing x) at the right, at the far end of the zoom range and at the near end. The test draws a map that is higher to the east with a high shelf along its north edge, at both ends, and expects the shelf at the top of the terrain in the centre column and the higher ground at the right of the centre row (docs/adr/0016-first-light-renderer.md).

**REN-024** `design` `test:render_grid_fit_is_a_pure_function_of_cells_and_limits`
Whether a grid fits a device MUST be a pure function of the two cell counts and the device's maximum buffer size given as an integer: it MUST return an error value when the vertex buffer (24 bytes per vertex) or the index buffer (4 bytes per index, six per cell) would exceed that size, computed without overflow, and Ok otherwise. Under the default 256 MiB limit a grid of 4096 cells a side is refused and one of 2048 cells a side is accepted, both decided without building the grid (docs/adr/0016-first-light-renderer.md).

**REN-025** `design` `test:conformance_gpu_upload_refuses_a_grid_over_the_device_limit`
The renderer MUST run the fit check of REN-024 against the device's own maximum buffer size before it creates any buffer and MUST return its error, so no well-formed call reaches wgpu's default error handler, which panics; it MUST request its device with the default limits lowered to what the adapter supports, never above them, and MUST refuse a mesh whose buffers do not match its cell counts. The test uploads a mesh claiming 4096 cells a side and one with too few vertices (docs/adr/0016-first-light-renderer.md).

**REN-026** `design` `test:render_missing_adapter_is_an_error_value`
A missing adapter and a refused device request MUST each be an error value naming what failed, never a panic. The test reaches the first through the constructor that takes the backends to search, given an empty set; the second is implemented by returning the device request's error as its own variant and no test reaches it, because a request made inside an adapter's own limits cannot be made to fail without the arguments wgpu documents as panics (docs/adr/0016-first-light-renderer.md).

**REN-027** `design` `test:conformance_gpu_misuse_is_an_error_value`
Drawing before a mesh is uploaded, and drawing into a target with a zero dimension, MUST each be an error value and not a call into wgpu. The test expects both after a well-formed draw has succeeded (docs/adr/0016-first-light-renderer.md).

**REN-028** `design` `test:render_gpu_gate_decides_over_four_cases`
A `conformance_gpu_` test MUST pass printing `SKIP: <reason>` when no adapter exists, MUST fail with the same reason when `SUPERCOM_REQUIRE_GPU=1` is set and none exists, and MUST print the adapter's name and backend when it runs. The decision is a pure function of whether an adapter was found and whether the flag is set, tested over all four cases; the printed skip line itself cannot be reached on a host that has an adapter (docs/adr/0016-first-light-renderer.md).

**REN-029** `design` `test:render_shader_source_is_assembled_from_the_rust_constants`
The shader source MUST be returned by a pure function that writes the light direction, the ambient share, and the three ramp stops from the crate's Rust constants, so no shading number is typed in both languages, and MUST name the four entry points the pipelines use. The test checks the source for each constant's text and each entry point; that the source compiles and shades correctly is REN-021's test (docs/adr/0016-first-light-renderer.md).

**REN-030** `game-data` `test:conformance_gpu_installed_dry_map_shows_only_terrain`
At the strategic view of the installed map SCMP_012, drawn into a square target, the middle 80 percent of the frame's width and of its height MUST hold only terrain colours, neither the clear colour nor the water's, and no pixel of the frame may be the water's, because the map carries no water.

**REN-031** `game-data` `test:conformance_gpu_installed_wet_map_shows_its_water_share`
At the strategic view of the installed map SCMP_016, drawn into a square target, the water's colour MUST cover a fraction of the pixels that are not the clear colour within 0.03 of 0.095. The heightmap's share of samples at or below the stored water elevation 17.5 is 0.0954, and only 17 of its 66,049 samples sit exactly at that elevation, so ties in depth do not move the fraction.

## The windowed mode

The `supercom` binary gains one subcommand, `view`. Its decisions are pure
functions in the binary, tested without a window; the one test that opens a
window runs only when `SUPERCOM_REQUIRE_DISPLAY=1` is set. The names below
are those of docs/adr/0016-first-light-renderer.md.

**REN-032** `design` `test:supercom_parse_args_cases`
The command line MUST be parsed by a pure function of the argument list, before anything else is done, so a usage error is reported whatever the display or the install. No argument MUST be the bare run, which prints the version line and whether the game data was found, exactly as before the windowed mode. `view` MUST take an optional map folder, `SCMP_012` when none is given, and an optional `--frames <n>` with n a positive integer of ASCII digits that fits 32 bits, in either order. The map folder MUST be a single non-empty path component: it contains no `/` or `\` and is neither `.` nor `..`. Any other argument list MUST print the usage to stderr and return the failure code before the install is read. The test parses each accepted form, with the empty list parsing to the bare run, and the rejected ones, among them an unknown option, a repeated `--frames`, two folders, a zero, signed, fractional, or oversized count, and folders with a separator or a dot component (docs/adr/0016-first-light-renderer.md).

**REN-033** `design` `test:supercom_view_without_display_fails_cleanly`
`view` MUST create the window system's event loop first, before the install is read. Where no display exists the creation fails, and the binary MUST then print to stderr a message that says no display is available and names `WAYLAND_DISPLAY`, `WAYLAND_SOCKET`, and `DISPLAY`, the variables the window system reads, and return the failure code without panicking. On Linux, the one platform where a display is found only through those variables, the test runs the built binary with `view` and those three variables removed from its environment and expects exit code 1, the message, and no mention of the install, which it points at a path that does not exist (docs/adr/0016-first-light-renderer.md).

**REN-034** `design` `test:supercom_choose_map_file_cases`
The map of a folder MUST be the one file in `<install>/maps/<folder>/` whose name ends `.scmap`, ignoring case; a folder with no such file, or with more than one, MUST be an error value naming the folder. The choice is a pure function of the folder's file names, tested over a match among other files, a match in capitals, near misses that only contain the suffix, an empty list, and two matches (docs/adr/0016-first-light-renderer.md).

**REN-035** `design` `test:supercom_plan_run_failures`
A run of `view` MUST proceed in this order: the display (REN-033), the install, resolved as the bare run resolves it, the map file (REN-034), its parse, the window, the renderer for the window's surface, the grid fit check against that renderer's device, the terrain mesh, the water plane, the camera, the upload, and the loop. The fit check, REN-024, MUST run before the mesh is built, so a map the device cannot hold ends the run with its error and the mesh is never allocated. Each failure MUST end the run with its text on stderr and the failure code. The steps up to the window are one pure-in-effect function of the install directory and the folder; its test creates directories under the system's temporary directory, removes them, and expects the failure of each step: an unresolved install, a missing install, a folder that cannot be listed, none and two map files, and a file that does not parse. Its success on an installed map is exercised by a conformance test (docs/adr/0016-first-light-renderer.md).

**REN-036** `design` `test:supercom_input_drives_the_camera`
The binary MUST turn the window system's events into its own input type, and one pure function MUST apply an input to the camera. A wheel of one line MUST zoom one step, away from the user toward the near end and toward the user toward the far end, at the normal speed, or the fast speed while shift is held; in the binary's input type a positive line count is a wheel turned away from the user. A pixel-delta wheel MUST count 40 pixels to the line, fractionally. WASD and the arrow keys MUST pan the target by 0.04 of the camera's distance per key event, north, west, south, and east as on the map (north is decreasing z); Home MUST go to the strategic view; Escape MUST close the window. Any other key is no input. The test applies each to a camera and expects the distance and target it implies, and expects a wheel count that is not finite to leave the camera unchanged (docs/adr/0016-first-light-renderer.md).

**REN-037** `design` `test:render_camera_ignores_non_finite_input`
`Camera::zoom` and `Camera::pan` MUST ignore input that is not finite and leave the camera exactly as it was. The test passes NaN and both infinities to each, and is pinned by two mutants of the camera's source, each removing one of the two guards, which make it fail (docs/adr/0016-first-light-renderer.md).

**REN-038** `design` `test:supercom_frame_plan_over_sizes`
Whether a frame is drawn MUST be a pure function of the window's size and the size the surface is configured at. When either dimension is zero, as when the window is minimized, the binary MUST leave the surface and the camera as they are and draw nothing until the size is positive again, and that is not an error. Otherwise it MUST draw, configuring the surface with the renderer's device first when the size differs from the configured one, and setting the camera's aspect ratio to the size's width over height; an error from setting the aspect ratio is reported once on stderr and is not fatal. A surface found lost or outdated MUST be configured again; a frame that did not present MUST be counted, and after 30 in a row the run ends with a failure; running out of memory ends the run with a failure. The counting is a pure type, tested beside the plan (docs/adr/0016-first-light-renderer.md).

**REN-039** `design` `test:conformance_window_presents_frames`
`view <folder> --frames <n>` MUST present n frames, print `presented <n> frames` to stdout, and return the success code. The test runs the built binary on `SCMP_012` with `--frames 3` only when `SUPERCOM_REQUIRE_DISPLAY=1` is set, with the environment it inherits, display variables included, so it is run only inside a nested compositor that shows nothing; without the flag it prints `SKIP: <reason>` and passes, whether or not a display exists. It waits at most a minute and then kills the binary and fails (docs/adr/0016-first-light-renderer.md).

**REN-040** `design` `test:render_surface_format_prefers_srgb_then_first`
The format the renderer draws in for a surface MUST be a pure function of the formats the surface offers, in their order: the first sRGB format when there is one, otherwise the first format, and an error value when the surface offers none. The surface constructor MUST use it (docs/adr/0016-first-light-renderer.md).

**REN-041** `design` `test:conformance_gpu_guarded_calls_return_errors_as_values`
The renderer MUST offer a public way to run a caller's wgpu calls inside its validation and out-of-memory error scopes and to get a captured error back as an error value, so that the window's surface configuration, texture acquisition, and presentation never report to the device's default handler, which panics. The test makes a call that raises nothing and expects its value, makes a texture of zero width inside the scope and expects the error value, and then draws (docs/adr/0016-first-light-renderer.md).

**REN-042** `design` `test:render_steep_terrain_is_an_error`
Building the terrain MUST return an error value, and MUST NOT panic, for a map whose slope leaves a vertex normal that is not a finite vector of unit length, either because an elevation difference is so large that its square overflows or because the cell is so small that the slope does. The test builds one map of each kind and a gentle map that still builds (docs/adr/0016-first-light-renderer.md).
