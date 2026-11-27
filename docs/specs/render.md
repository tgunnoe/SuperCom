# Render specification

This file specifies the first slice of the renderer. Its first part needs no
graphics device: the terrain grid built from a parsed map, the water plane, the
camera with its zoom range and clip planes, the height normalization, and the
shading function, all pure functions over plain `f32` arrays in the crate
`supercom-render`. Its second part is the GPU pipeline in the same crate, which
draws that terrain and water for a camera into any texture view through wgpu;
it needs an adapter but no window, and its tests draw offscreen. Its third part
is the windowed mode, the `view` subcommand of the `supercom` binary, which
opens a window on an installed map and calls the pipeline's one draw function;
its decisions are pure functions of the binary, tested without a window. Its
fourth part is the models and instances the same pipeline draws over the
terrain and the water: plain data placed by the caller, specified from the
checks that refuse them to the pixels they leave. Its fifth part is the replay
run, the `replay` subcommand of the same binary, which plays a recorded match
in the window: the steps before the window, the stepping and its comparison
with the log's checkpoints, the pacing and the interpolation, and what is
drawn, every entity and projectile as a flat marker over the terrain. Its sixth
part is the unit meshes: the kinds a run resolves, read from the game's own
lod0 meshes before the window, the model built from a mesh, and the rule that
draws a unit as its lit mesh when the camera is near and as its marker
otherwise. Its camera and shading are decisions of this project, recorded in
docs/adr/0016-first-light-renderer.md (the near end of the zoom range in
docs/adr/0017-units-on-the-map.md): a search of the installed data finds no
number for a world camera's field of view, clip planes, or zoom limits, so no
clause below says how the original behaves.

The frontier of this slice: the GPU pipeline's and the windowed mode's
contracts are specified below; the window loop itself, the code that turns the
window system's events into inputs and draws, is reached by no headless test
and is tried only when a display is available, so a person's run is its
evaluator (the record's section Known at ratification says what the run at its
ratification covered); a graphics device lost by the driver is reported by
nothing (issue 74 of https://github.com/tgunnoe/SuperCom); the forge's runner
has no graphics adapter, so every `conformance_gpu_` test skips there and
REN-029's parse and validation are the only check the shader gets (issue 77 of
https://github.com/tgunnoe/SuperCom); two sentences, REN-035's order of the fit
check before the mesh and REN-040's last, are reached by no test (issue 78 of
https://github.com/tgunnoe/SuperCom); the three installed maps of 4096 cells a
side are refused and not drawn (issue 69); decals, shadows,
water effects, fog of war, strategic icons, and post-processing are later
slices, and so are textures, animation, and levels of detail for unit meshes: a
mesh is drawn untextured, at rest, from its lod0 geometry alone (the terrain's
own ground textures and masks are the section The textured terrain; what it
still leaves out is stated there); the renderer's
refusal of a short normal (REN-086) ends a run whose kinds include UXB0008 or
UXB0009, and mending installed normals is not done; the frame the replay run's
window draws is tried in a nested compositor and by a person, while its
stepping, its pacing, and its instance list are pure and tested without a
window; and the vertical reading of REN-001 awaits a comparison with the
running original, the first look at the result, on SCMP_012 and SCMP_016,
having raised no mismatch. The window's two calls that hand the renderer the
model set and the instance list (REN-083, REN-088) are reached by no test: the
window opens at the strategic view, where every unit of the committed log is a
marker, and no test zooms; the close-zoom pixel test covers everything up to
those calls, and the head's look of 2026-11-18
closed them: zooming in, markers became meshes.
Structures are drawn facing east, where the head reports that the original
builds them facing south (issue 93 of https://github.com/tgunnoe/SuperCom);
whether a unit is drawn as the mirror image of itself in the original is
unobserved (AGENTS.md U11); and at the very end of a recording REN-076 can count
a frame as held back with nothing held (issue 90 of
https://github.com/tgunnoe/SuperCom).

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
The water plane MUST be one quad, facing up, at the stored surface elevation, spanning the header's extent, when the map carries water, and absent when it carries none, when its stored elevation is not finite, or when the header's extent is not finite and positive (docs/adr/0016-first-light-renderer.md).

## Camera

**REN-009** `design` `test:render_target_projects_to_centre_on_highest_plane`
The camera's view-projection MUST project the camera's target to the centre of the screen; the target MUST lie on the horizontal plane at the map's highest vertex elevation, and the eye MUST never be below that plane. The vertical field of view is 45 degrees, a choice of this project (docs/adr/0016-first-light-renderer.md).

**REN-010** `design` `test:render_zoom_range_far_end_is_the_strategic_view`
The zoom range MUST be a distance interval, serving the continuous tactical-to-strategic zoom HUD-011 of docs/specs/ui.md requires. Its far end MUST be the smallest distance at which, looking straight down with the target at the map's centre, the whole map extent on the target's plane is inside the view for the given aspect ratio (the strategic view); inside the view means clip-space x and y no larger than w in magnitude, compared with a relative tolerance of 1e-4, because at the far end the extent's corners lie on the screen's edge by construction. Its near end MUST be 8 world units. Zooming MUST clamp to the range, the distance MUST be continuous within it, and one step MUST move the distance by 0.02 of the range at normal speed and 0.08 at fast speed, a positive number of steps zooming toward the strategic end and a fractional number zooming by that fraction of a step (docs/adr/0016-first-light-renderer.md, docs/adr/0017-units-on-the-map.md).

**REN-011** `design` `test:render_zoom_out_from_a_corner_shows_the_whole_extent`
At the far end of the zoom range the whole extent MUST be in view, whatever the target was before zooming out, so HUD-011 of docs/specs/ui.md always has its strategic view. The target's clamp narrows with the distance (REN-013) and is the map's centre at the far end, so zooming out moves a target toward the centre. The test zooms out by steps from a target at each corner to the far end and expects the extent's four corners, at the highest vertex elevation, to project inside the screen, and expects that at a distance one percent smaller, with the target at the centre, at least one corner does not (docs/adr/0016-first-light-renderer.md).

**REN-012** `design` `test:render_pitch_tilts_continuously_with_north_up_east_right`
The camera MUST look straight down at the far end of the zoom range and tilt toward a pitch of 35 degrees at the near end, the pitch (the angle of the view direction below the horizontal, 90 degrees when straight down) falling linearly with the distance from 90 degrees at the far end to 35 degrees at the near end. The view matrix MUST be finite at every zoom, the straight-down end included, with north (decreasing z) up on screen and east to the right: at the far end and at the near end, a point east of the target has a larger clip-space x than the target, and a point north of it a larger clip-space y (docs/adr/0016-first-light-renderer.md).

**REN-013** `design` `test:render_pan_clamps_target_inside_narrowed_extent`
Panning MUST move the target in x and z and clamp it to the map's extent as narrowed by the zoom: the allowed target region is the extent scaled about the map's centre by (far - distance) / (far - near) of the zoom range, the whole extent at the near end and the centre alone at the far end (docs/adr/0016-first-light-renderer.md).

**REN-014** `design` `test:render_clip_planes_bound_every_visible_vertex`
The near and far clip planes MUST be functions of the distance such that, at every allowed zoom and target, every vertex of the map that lies inside the view pyramid (inside the frustum's four side planes) has a depth between the near and far planes, and the eye is higher than the map's highest vertex. The near plane is half the smallest depth at which a point on the target's plane or below can lie inside the pyramid, and the far plane is 1.1 times a bound on the distance from the eye to any point of the extent between the map's lowest and highest elevations: the hypotenuse of the extent's diagonal plus the eye's horizontal offset from the target and of the eye's height above the target's plane plus the relief (docs/adr/0016-first-light-renderer.md).

**REN-016** `design` `test:render_aspect_change_recomputes_range_and_reclamps`
Setting the aspect ratio MUST recompute the zoom range and re-clamp the distance and the target; an aspect ratio that is not finite and positive MUST be refused and MUST leave the camera unchanged, and so MUST one at which the extent's strategic view would be no farther than the near end of the zoom range (docs/adr/0016-first-light-renderer.md).

**REN-017** `design` `test:render_camera_rejects_bad_inputs`
Constructing a camera MUST return an error value, never panic, for an extent or an aspect ratio that is not finite and positive, an elevation that is not finite, a relief (highest minus lowest) that is not finite, a highest elevation below the lowest, or an extent so small that its strategic view is no farther than the near end of the zoom range; a flat map, highest equal to lowest, is valid (docs/adr/0016-first-light-renderer.md).

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
For a terrain drawn by the fallback (REN-111), the height ramp, a terrain pixel MUST hold the colour of the shading function (REN-015) for that fragment: the elevation interpolated to the fragment normalized over the mesh's own range by the normalization of REN-015, a range of zero width included, and the interpolated normal renormalized; not an interpolation of colours computed at the vertices, which cannot follow the ramp across its middle stop. The light direction, the ambient share, and the three ramp stops in the shader source MUST be written from the crate's Rust constants. The colour MUST be encoded with the target format's transfer function, the sRGB encoding for an sRGB format and the identity for a Unorm format, and MUST be within 2 in 255 per channel of that. The test draws four synthetic planes (rising east, rising south, rising diagonally, flat) in Rgba8UnormSrgb, Bgra8UnormSrgb, and Rgba8Unorm, finds for each sampled pixel the surface point its centre sees, and compares the pixel with the colour the pure function gives there, over normalized heights from below 0.2 to above 0.8 and four distinct normals (docs/adr/0016-first-light-renderer.md).

**REN-022** `design` `test:conformance_gpu_water_hides_only_terrain_below_its_surface`
The water plane MUST be drawn at its stored elevation, opaque, in the linear RGB colour (0.04, 0.22, 0.45) with no lighting, with depth testing against the terrain, so terrain below the surface shows the water's colour and terrain above it shows its own. The water's colour MUST be told from every colour of the height ramp by its blue channel exceeding both its red and its green. The test draws a plane that crosses the surface and samples both sides (docs/adr/0016-first-light-renderer.md).

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
The shader source MUST be returned by a pure function that writes the light direction, the ambient share, and the three ramp stops from the crate's Rust constants, so no shading number is typed in both languages, and MUST name the four entry points the pipelines use. The test checks the source for each constant's text and each entry point, parses and validates it with naga's WGSL front end, reached through wgpu's re-export, and checks the uniform struct's layout, so the source's compilation is checked without an adapter; that it shades correctly is REN-021's test (docs/adr/0016-first-light-renderer.md).

**REN-030** `game-data` `test:conformance_gpu_installed_dry_map_shows_only_terrain`
At the strategic view of the installed map SCMP_012, drawn into a square target, the middle 80 percent of the frame's width and of its height MUST hold only terrain colours, neither the clear colour nor the water's, and no pixel of the frame may be the water's, because the map carries no water.

**REN-031** `game-data` `test:conformance_gpu_installed_wet_map_shows_its_water_share`
At the strategic view of the installed map SCMP_016, drawn into a square target, the water's colour MUST cover a fraction of the pixels that are not the clear colour within 0.03 of 0.095. The heightmap's share of samples at or below the stored water elevation 17.5 is 0.0954, and only 17 of its 66,049 samples sit exactly at that elevation, so ties in depth do not move the fraction.

## Models and instances

The renderer draws models: plain positions, normals, and triangle indices, and
instances of them, each a model's index in the uploaded set, a transform, a
colour, and a style. It knows nothing of what a model depicts. The names are
those of docs/adr/0017-units-on-the-map.md.

**REN-043** `design` `test:render_model_check_refuses_each_malformed_model`
A model MUST be positions and normals of equal count and 32-bit triangle indices, three to a triangle, in the terrain's own vertex layout (REN-024: a position then a normal, 24 bytes). The pure check of a model, made before any buffer exists, MUST return an error value, each its own and each naming the model's index in its set, for a model with no vertex or no index, for unequal position and normal counts, for an index count that is not a multiple of three, for an index at or past the vertex count, and for a position or a normal that is not finite, checked in that order. The test checks one model of each kind and a well-formed one (docs/adr/0017-units-on-the-map.md).

**REN-044** `design` `test:conformance_gpu_model_upload_replaces_the_set_and_a_refusal_changes_nothing`
Uploading a set of models MUST run the check of REN-043 and the fit of REN-045 on every model before it creates any buffer. A set that uploads MUST replace the set before it and empty the instance list; a set that is refused MUST leave the set and the list as they were. The test uploads a set and a list and draws, then uploads sets with one malformed model of each kind and expects each error, the same frame, and the same set, then uploads a good set and expects the list empty (docs/adr/0017-units-on-the-map.md).

**REN-045** `design` `test:render_model_and_instance_fit_is_a_pure_function_of_counts_and_limits`
Whether a model's buffers, and an instance list's buffer, fit a device MUST be a pure function of the counts and the device's maximum buffer size given as an integer, computed without overflow and used before any buffer exists, as REN-024 is for the grid. A model's vertex buffer MUST take 24 bytes per vertex and its index buffer 4 bytes per index, and an instance list's buffer 76 bytes per instance, sixteen 32-bit floats of transform and three of colour; a buffer over the size MUST be an error value naming it, the vertex buffer before the index buffer. The test pins each byte count and the boundary at its exact size, and a count that would overflow (docs/adr/0017-units-on-the-map.md).

**REN-046** `design` `test:render_instance_check_refuses_each_bad_instance`
An instance MUST be a model named by its index in the uploaded set, a column-major transform in the convention of REN-008 taking model space to world space, a linear RGB colour, and a style, lit or flat. Setting the instance list MUST refuse, each as its own error value naming the instance's position in the list, an instance that names no model of the set, a transform with a value that is not finite, and a colour channel that is not finite or lies outside 0 to 1, the first offender in list order; a pure check of a list against the set's model count makes these decisions without a device. The test checks each refusal, the channels 0 and 1 as valid, and the order (docs/adr/0017-units-on-the-map.md).

**REN-047** `design` `test:conformance_gpu_instance_list_stays_until_replaced`
An empty instance list MUST be valid, and a list MUST stay, drawn by every frame, until another list is set or a set of models is uploaded; a list that is refused MUST leave the list as it was, and with no model set every non-empty list is refused. The test sets a list, draws twice, sets lists that are refused and draws, sets an empty list and draws, and sets a new list and draws (docs/adr/0017-units-on-the-map.md).

**REN-048** `design` `test:conformance_gpu_flat_is_never_hidden_and_lit_is_hidden_by_nearer_terrain`
The one draw function of REN-019, its signature unchanged, MUST draw the terrain, the water, then the lit instances, then the flat instances. A lit instance MUST be drawn with the depth test Less and depth writes, back faces culled and counter-clockwise faces in front, so terrain nearer than it hides it and a face turned from the eye is not drawn. A flat instance MUST be drawn with no depth test, no depth write, and no culling, in the colour as given, so it is never hidden by terrain, by a lit instance, or by its own back face. The test draws lit and flat instances above and below flat ground and with a down-facing model, and samples each (docs/adr/0017-units-on-the-map.md).

**REN-049** `design` `test:conformance_gpu_an_empty_instance_list_leaves_the_picture_unchanged`
A frame drawn with no model set, and one drawn with a model set and an empty instance list, MUST be equal pixel for pixel for the same scene, camera, and size, and so MUST a frame drawn after a list is emptied. The test draws a scene with water at the strategic view and at the near end, compares the frames, and first expects a one-instance list to change the picture, so that the comparison can fail (docs/adr/0017-units-on-the-map.md).

**REN-050** `design` `test:render_placement_scales_then_turns_then_translates`
The placement transform MUST be a pure function of a position, a heading, a uniform scale, and the model's own forward yaw that scales, then turns about the y axis, then translates. Headings and yaws are measured as the simulation measures a heading, 0 along +x and increasing toward +z, and the turn MUST take the model's forward direction onto (cos h, 0, sin h) for heading h; with f the model's forward yaw, it takes every horizontal direction (cos a, 0, sin a) to (cos(a + h - f), 0, sin(a + h - f)) and leaves the y axis alone. Because +z is south and y is up, this is the opposite of the usual positive turn about +y by that angle. The test pins the sign with literals: with yaw 0, heading 0 leaves the model's +x along +x and heading pi/2 takes it to +z, and with yaw pi/2 and heading 0 the model's +z lies along +x; and pins the order with one composite: the model point (1, 0, 0) under scale 2, heading pi/2, yaw 0, and position (10, 5, 20) maps to (10, 5, 22) within 1e-5 (docs/adr/0017-units-on-the-map.md).

**REN-051** `design` `test:render_placement_refuses_what_could_reverse_a_winding`
The placement transform MUST return an error value, not a matrix, when the scale is not finite and positive, or when the heading, the yaw, or a coordinate of the position is not finite, so that no transform it returns reverses a triangle's winding. The test refuses each input in turn with NaN and both infinities, a zero scale, and a negative scale, and expects a positive determinant of the upper-left three by three of what it does return (docs/adr/0017-units-on-the-map.md).

**REN-052** `design` `test:render_lit_shading_is_ambient_plus_the_light_share`
The lit shading function MUST map an instance's linear colour and a unit normal to the colour times (AMBIENT + (1 - AMBIENT) * max(n . L, 0)), with the terrain's own light direction (-0.36, 0.8, -0.48) and ambient share 0.3 (REN-015). The test pins it with literals for the colour (0.5, 0.5, 0.5): along the light direction the colour is unchanged, at (0.8, 0, -0.6), perpendicular to the light, it is multiplied by 0.3, and at (0.36, -0.8, 0.48), facing away, it is also multiplied by 0.3 (docs/adr/0017-units-on-the-map.md).

**REN-053** `design` `test:conformance_gpu_lit_pixels_match_the_lit_shading_function`
A lit instance's normal MUST be the model's normal turned by the upper-left three by three part of its transform and then renormalized, which is exact for the transforms of REN-050. A lit pixel MUST equal the colour of REN-052's function for its face, encoded with the target's transfer function, within 2 in 255 per channel, and a flat pixel MUST equal the instance's colour, encoded the same way. The test places a tilted model with a uniform scale of 0.25 and a heading of pi/2, where an unturned or an unrenormalized normal gives another colour, and compares a lit and a flat pixel with literals worked out from the clauses (docs/adr/0017-units-on-the-map.md).

**REN-054** `design` `test:render_model_shader_source_is_a_second_module_assembled_from_the_constants`
The model shader's source MUST be returned by a pure function of its own, a second module apart from the source of REN-021 and REN-029, which stays the terrain and water module with its four entry points. It MUST write the light direction and the ambient share from the crate's Rust constants, so that no shading number is typed in both languages, and MUST name its four entry points, `vs_lit`, `fs_lit`, `vs_flat`, and `fs_flat`. The lit and flat pipelines MUST read the camera's view-projection matrix as given, as REN-019 requires of the shader's uniform. The test parses and validates the source with naga's WGSL front end, reached through wgpu's re-export, checks the layout of its uniform struct as REN-029's test does, and checks that neither module names the other's entry points (docs/adr/0017-units-on-the-map.md).

**REN-055** `design` `test:conformance_gpu_model_heading_points_right_at_zero_and_down_at_a_quarter_turn`
At the strategic view, with north up on the screen (REN-023), a model that extends along its own +x and is placed with yaw 0 MUST extend toward the right of the frame at heading 0 and toward the bottom at heading pi/2. The test draws an asymmetric flat model at both headings and samples a point along its tip and the point as far behind its base (docs/adr/0017-units-on-the-map.md).

**REN-056** `design` `test:conformance_gpu_terrain_upload_leaves_the_models_and_the_list`
The terrain upload MUST leave the model set and the instance list as they were, and a model set and a list MAY be set before or after the terrain is uploaded; a draw with no terrain uploaded stays REN-027's error whatever else is set. The test sets a model set and a list, draws and expects the error, uploads the terrain, draws and expects the instance's colour at its projected pixel, then uploads the terrain again, draws, and expects that pixel again (docs/adr/0017-units-on-the-map.md).

## The windowed mode

The `supercom` binary gains two subcommands, `view` and, in the replay run
below, `replay`. Their decisions are pure functions in the binary, tested
without a window; the tests that open a window run only when
`SUPERCOM_REQUIRE_DISPLAY=1` is set. The names below are those of
docs/adr/0016-first-light-renderer.md.

**REN-032** `design` `test:supercom_parse_args_cases`
The command line MUST be parsed by a pure function of the argument list, before anything else is done, so a usage error is reported whatever the display or the install. No argument MUST be the bare run, which prints the version line and whether the game data was found, exactly as before the windowed mode. `view` MUST take an optional map folder, `SCMP_012` when none is given, and an optional `--frames <n>` with n a positive integer of ASCII digits that fits 32 bits, in either order. The map folder MUST be a single non-empty path component: it contains no `/` or `\` and is neither `.` nor `..`. An argument that begins with `-` is an option, so a map folder never begins with `-`. `replay` MUST take one file path, required, and an optional `--speed <x>` and an optional `--frames <n>` as for `view`, in any order; the path is a non-empty argument that does not begin with `-`; the speed is written as one or more ASCII digits, optionally followed by a point and one or more ASCII digits, with no sign and no exponent, and lies from 0.1 to 10 inclusive, and is 1 when absent. An option given twice, a second path, a missing path, a missing value, or a speed outside the bounds is a usage error. Any other argument list MUST print the usage to stderr and return the failure code before the install is read. Every argument list accepted before `replay` was added parses as before, and every list rejected before still fails unless it begins with `replay`. The test parses each accepted form, with the empty list parsing to the bare run, and the rejected ones, among them an unknown option, a repeated `--frames`, two folders, a zero, signed, fractional, or oversized count, and folders with a separator or a dot component; for `replay` it parses the forms in each order, the speeds 0.1 and 10 as the bounds, and rejects 0.09 and 10.01 as just past them, a signed, exponent, empty, or doubly pointed speed, and each of the usage errors above (docs/adr/0016-first-light-renderer.md and docs/adr/0017-units-on-the-map.md).

**REN-033** `design` `test:supercom_view_without_display_fails_cleanly`
`view` MUST create the window system's event loop first, before the install is read. Where no display exists the creation fails, and the binary MUST then print to stderr a message that says no display is available and names `WAYLAND_DISPLAY`, `WAYLAND_SOCKET`, and `DISPLAY`, the variables the window system reads, and return the failure code without panicking. On Linux, the one platform where a display is found only through those variables, the test runs the built binary with `view` and those three variables removed from its environment and expects exit code 1, the message, and no mention of the install, which it points at a path that does not exist (docs/adr/0016-first-light-renderer.md).

**REN-034** `design` `test:supercom_choose_map_file_cases`
The map of a folder MUST be the one file in `<install>/maps/<folder>/` whose name ends `.scmap`, ignoring case; a folder with no such file, or with more than one, MUST be an error value naming the folder. The choice is a pure function of the folder's file names, tested over a match among other files, a match in capitals, near misses that only contain the suffix, an empty list, and two matches (docs/adr/0016-first-light-renderer.md).

**REN-035** `design` `test:supercom_plan_run_failures`
A run of `view` MUST proceed in this order: the display (REN-033), the install, resolved as the bare run resolves it, the map file (REN-034), its parse, the window, the renderer for the window's surface, the map's ground textures and masks, resolved and decoded as REN-111 to REN-116 have it, the grid fit check against that renderer's device, the terrain mesh, the water plane, the camera, the upload, and the loop. The fit check, REN-024, MUST run before the mesh is built, so a map the device cannot hold ends the run with its error and the mesh is never allocated. Each failure MUST end the run with its text on stderr and the failure code. The steps up to the window are one pure-in-effect function of the install directory and the folder; its test creates directories under the system's temporary directory, removes them, and expects the failure of each step: an unresolved install, a missing install, a folder that cannot be listed, none and two map files, and a file that does not parse. Its success on an installed map is exercised by a conformance test (docs/adr/0016-first-light-renderer.md).

**REN-036** `design` `test:supercom_input_drives_the_camera`
The binary MUST turn the window system's events into its own input type, and one pure function MUST apply an input to the camera. A wheel of one line MUST zoom one step, away from the user toward the near end and toward the user toward the far end, at the normal speed, or the fast speed while shift is held; in the binary's input type a positive line count is a wheel turned away from the user. A pixel-delta wheel MUST count 40 pixels to the line, fractionally. WASD and the arrow keys MUST pan the target by 0.04 of the camera's distance per key event, north, west, south, and east as on the map (north is decreasing z); Home MUST go to the strategic view; Escape MUST close the window. Any other key is no input. The test applies each to a camera and expects the distance and target it implies, and expects a wheel count that is not finite to leave the camera unchanged (docs/adr/0016-first-light-renderer.md).

**REN-037** `design` `test:render_camera_ignores_non_finite_input`
`Camera::zoom` and `Camera::pan` MUST ignore input that is not finite and leave the camera exactly as it was. The test passes NaN and both infinities to each and expects the camera unchanged (docs/adr/0016-first-light-renderer.md).

**REN-038** `design` `test:supercom_frame_plan_over_sizes`
Whether a frame is drawn MUST be a pure function of the window's size and the size the surface is configured at. When either dimension is zero, as when the window is minimized, the binary MUST leave the surface and the camera as they are and draw nothing until the size is positive again, and that is not an error. Otherwise it MUST draw, configuring the surface with the renderer's device first when the size differs from the configured one, and setting the camera's aspect ratio to the size's width over height; an error from setting the aspect ratio is reported once on stderr and is not fatal. A surface found lost or outdated MUST be configured again; a frame that did not present MUST be counted, and after 30 in a row the run ends with a failure; running out of memory ends the run with a failure. The counting is a pure type, tested beside the plan (docs/adr/0016-first-light-renderer.md).

**REN-039** `design` `test:conformance_window_presents_frames`
`view <folder> --frames <n>` MUST present n frames, print `presented <n> frames` to stdout, and return the success code. The test runs the built binary on `SCMP_012` with `--frames 3` only when `SUPERCOM_REQUIRE_DISPLAY=1` is set, with the environment it inherits, display variables included, so it is run only inside a nested compositor that shows nothing; without the flag it prints `SKIP: <reason>` and passes, whether or not a display exists; with the flag set and no install found, it fails. It waits at most a minute and then kills the binary and fails (docs/adr/0016-first-light-renderer.md).

**REN-040** `design` `test:render_surface_format_prefers_srgb_then_first`
The format the renderer draws in for a surface MUST be a pure function of the formats the surface offers, in their order: the first sRGB format when there is one, otherwise the first format, and an error value when the surface offers none. The surface constructor MUST use it (docs/adr/0016-first-light-renderer.md).

**REN-041** `design` `test:conformance_gpu_guarded_calls_return_errors_as_values`
The renderer MUST offer a public way to run a caller's wgpu calls inside its validation and out-of-memory error scopes and to get a captured error back as an error value, so that the window's surface configuration, texture acquisition, and presentation never report to the device's default handler, which panics. The test makes a call that raises nothing and expects its value, makes a texture of zero width inside the scope and expects the error value, and then draws (docs/adr/0016-first-light-renderer.md).

**REN-042** `design` `test:render_steep_terrain_is_an_error`
Building the terrain MUST return an error value, and MUST NOT panic, for a map whose slope leaves a vertex normal that is not a finite vector of unit length, either because an elevation difference is so large that its square overflows or because the cell is so small that the slope does. The test builds one map of each kind and a gentle map that still builds (docs/adr/0016-first-light-renderer.md).

## The replay run

The `supercom` binary's second subcommand, `replay <file>`, plays a recorded
match in the window. It loads the world the log's header names, checks the log
against it, steps the simulation at ten ticks a second of wall time times the
speed, compares every checkpoint as it goes, and draws every entity and
projectile each frame as a flat marker over the terrain. The clauses of this
section speak of markers; the unit meshes follow in the next section. The
binary reads the simulation through its public surface only. The decisions are
pure functions in the binary, each tested without a window, except the clauses
that name a display. The record is docs/adr/0017-units-on-the-map.md.

**REN-057** `design` `test:supercom_prepare_replay_failures`
A run of `replay` MUST proceed in this order: the display (REN-058), the install, resolved as the bare run resolves it, the replay file and its parse, the map folder the replay's header names, held to REN-032's single-component rule, the cache directory (REN-059), the world, loaded from the install into that directory, the content check and the start state, which are the check the simulation's replay start makes of the header's content digests against the world and the skirmish it then starts, the map file and its parse as REN-035 has them for `view`, the unit kinds of REN-082, resolved as REN-084 has it, and then the window, what `view` builds, the model set (REN-083), and the loop. A replay with no checkpoint MUST be refused after its parse, since the run would have no end. Every step before the window MUST be one function of the install directory, the file, and the cache variables, returning the parsed map, the replay, the start state, and the resolved kinds, or an error value whose text names the failing step, and each failure MUST end the run with its text on stderr and the failure code. The test creates directories under the system's temporary directory, removes them, and expects the failure of each step that needs no installed game: an unresolved install, a missing install, an unreadable file, a file that does not parse, a replay with no checkpoint, a header map folder that is not a single component, an unresolved cache directory, a relative one, and a world that cannot be loaded from an install holding nothing; it also expects the refusal of an install whose archives do not mount, for the step after the map file (docs/adr/0017-units-on-the-map.md).

**REN-058** `design` `test:supercom_replay_without_display_fails_cleanly`
`replay` MUST create the window system's event loop first, as REN-033 has it for `view`, after the command line is parsed and before the install or the replay file is read. Where no display exists it MUST print the message of REN-033 to stderr and return the failure code without panicking, and a usage error MUST be reported whatever the display. On Linux the test runs the built binary with `replay` and a path that does not exist, the install pointed at a path that does not exist, and the three display variables removed, and expects exit code 1, the message naming the three variables, and no mention of either path; and runs it with a speed past the bound and expects the usage and no display message (docs/adr/0017-units-on-the-map.md).

**REN-059** `design` `test:supercom_cache_dir_cases`
The directory the world's importer caches into MUST be a pure function of three environment values, each of which counts as unset when it is empty: `SUPERCOM_CACHE_DIR`, which names the directory itself; else `XDG_CACHE_HOME` joined with `supercom`; else the home directory joined with `.cache/supercom`. A directory that is not an absolute path from any of them MUST be refused, and so MUST an environment that names none; nothing falls back to the working directory. The binary does not create the directory: the importer does when it is missing. The test passes each case and expects the literal path, the first rule winning over the others and an empty value falling through, and expects the refusal of a relative path from each of the three and of an environment with none (docs/adr/0017-units-on-the-map.md).

**REN-060** `design` `test:supercom_checkpoint_verdict_cases`
The comparison of a step with the log MUST be a pure function of a tick, the digest the step run at that tick returned, and the checkpoint list, under the committed rule: the checkpoint at tick t is compared with the digest returned by the step run while the state's tick was t, the t-th digest of the simulation's own replay run. It MUST answer a match, no checkpoint at this tick, or a mismatch naming the tick. The test expects each answer on a list of three checkpoints, a digest differing in its last byte, and ticks before, between, and after the checkpoints (docs/adr/0017-units-on-the-map.md).

**REN-061** `design` `test:supercom_stop_rule_ends_at_the_last_checkpoint`
Stepping MUST stop after the step run at the last checkpoint's tick: a step is taken at tick t only when t is not past that tick, so a log whose last checkpoint is at tick L takes L + 1 steps, and a log with no checkpoint takes none. The test expects the answer at the ticks 0, L - 1, L, and L + 1 and on an empty list (docs/adr/0017-units-on-the-map.md).

**REN-062** `design` `test:conformance_replay_loop_matches_the_committed_log`
The binary's own stepping loop, one owner of the state, MUST over the committed log crates/replay-harness/fixtures/skirmish_scmp_012.replay take 4,600 steps, match all 46 of its checkpoints, and return digests equal to those the simulation's replay run returns for the same log, so that their first divergence is none. The test reads the log by a path built from `CARGO_MANIFEST_DIR`, imports the world into a fresh directory under the system's temporary directory, and needs the install (docs/adr/0017-units-on-the-map.md).

**REN-063** `design` `test:conformance_replay_loop_names_the_tick_of_a_wrong_checkpoint`
A digest that differs from its checkpoint MUST end the stepping at once with an error value naming the tick and ending the run with the failure code, with no step after it and no summary line (REN-073). The test changes the digest of the checkpoint at tick 99 of the committed log, steps, and expects the error naming tick 99 on the hundredth step and no checkpoint matched before it (docs/adr/0017-units-on-the-map.md).

**REN-064** `design` `test:supercom_pacing_cases`
Pacing MUST be a pure function of the elapsed wall time in seconds since the run's clock started, the speed, and the ticks already run. The ticks owed are floor(elapsed times speed times 10) less the ticks run, never below zero; the ticks due in a frame are the owed ticks capped at 10; the ticks held back are the owed ticks less the due ones; and the fraction is the part of elapsed times speed times 10 after its floor. A time or a speed that is not finite or is negative owes nothing. Ticks held back stay owed and are stepped on later frames: the clock is never re-based. The speed's bounds, 0.1 to 10, are REN-032's. The test pins literals: nothing owed at time 0; 0.25 s at speed 1 gives 2 due and a fraction of 0.5; 1 s at speed 2 gives 10 due and 10 held back; the same with 10 ticks run gives 10 due and none held back; a speed of 0.1 over 10 s gives 10 due and none held; and a NaN, an infinite, and a negative time or speed owe nothing (docs/adr/0017-units-on-the-map.md).

**REN-065** `design` `test:supercom_interpolation_cases`
Interpolation MUST span exactly one tick: a drawn position is the kept position plus the difference between the current and the kept position times the fraction of REN-064, componentwise, and a drawn heading is the kept heading plus the difference taken the short way round, the difference wrapped into the half-open range from minus pi to pi, times the fraction. The kept values are those of every entity and projectile as they were before the last step taken in the frame; a frame that steps no tick keeps the values it had while the fraction advances; an entity or a projectile with no kept value is drawn at its current values. The test pins a position blend, a heading blend across the wrap with the literal 3.0 to -2.5 at one half giving 3.391592653589793, a blend with no wrap, and the keeping of a state's values by id (docs/adr/0017-units-on-the-map.md).

**REN-066** `design` `test:supercom_scene_instances_cases`
What is drawn MUST be one pure function of the state, the kept values, the fraction, the world's kinds, the terrain mesh, and the camera's distance, returning the instance list: one instance for every entity in the state, in id order, then one for every projectile, in id order, each flat, placed by the renderer's placement function at its interpolated position and heading with a yaw of 0 and the scale of REN-068, in the colour of REN-067. An entity the state no longer holds is not drawn; a projectile is interpolated by its id under the same rule as an entity. The marker model of an entity is the commander's when its kind's categories hold the string `COMMAND`, else the mobile one when its kind's motion is not none, else the static one; a blueprint the world does not know is drawn as a complete kind that does not move and is 1 world unit across; a projectile uses the projectile model. Nothing is drawn lit. The test builds a state of known kinds and expects the model, the colour, the transform, and the order of each instance, a removed entity absent, and an unknown blueprint (docs/adr/0017-units-on-the-map.md).

**REN-067** `design` `test:supercom_scene_colours_and_dimming`
Each army's colour MUST be an entry of a table of eight linear colours, every channel from 0 to 1: (0.9, 0.1, 0.1), (0.1, 0.3, 0.9), (0.1, 0.7, 0.2), (0.9, 0.8, 0.1), (0.6, 0.1, 0.8), (0.1, 0.8, 0.8), (0.9, 0.5, 0.1), and (0.9, 0.9, 0.9); army n takes entry (n - 1) mod 8, and army 0 takes the first. An entity that is not complete, of a kind the world knows, MUST be drawn in its army's colour times 0.4 plus 0.6 times its build progress, the progress being the simulation's, from 0 to 1; a complete entity is not dimmed. A projectile is drawn in the colour (1.0, 0.95, 0.5), undimmed. The test pins the table's entries, the repeat of army 9 as army 1, army 0, and the dimming at progress 0, one half, and a complete entity (docs/adr/0017-units-on-the-map.md).

**REN-068** `design` `test:supercom_marker_size_cases`
A marker's size in world units MUST be the larger of the unit's own size and 0.02 times the camera's distance, so that a marker does not shrink below a share of the view at the strategic zoom; the unit's own size is the larger side of the kind's footprint, with a missing footprint read as 1. A projectile's marker is 0.5 world units across at every distance. The test pins literals at both ends of the zoom range of REN-010, 8 and 309.02, the far end for a square map 256 units a side at an aspect of 1 (128 divided by the tangent of 22.5 degrees), for a footprint of 1 and of 8 (docs/adr/0017-units-on-the-map.md).

**REN-069** `design` `test:render_terrain_height_follows_the_split_of_a_cell`
The renderer MUST give the height of the drawn terrain under a point as a pure function of the terrain mesh, a method of the mesh: the height of the surface of the two triangles of the cell holding the point, split as REN-003 has it from the north-east vertex to the south-west one, interpolated linearly within the triangle, and no height for a point outside the extent from 0 to the mesh's width along x and from 0 to its height along z, edges included, or for a coordinate that is not finite. The test pins literals on a sloped cell on each side of the diagonal, on the diagonal, at a vertex, on the far edge, and outside (docs/adr/0017-units-on-the-map.md).

**REN-070** `design` `test:supercom_scene_height_by_layer`
An entity on the land or the seabed layer MUST be drawn at the height of the drawn terrain under its interpolated x and z (REN-069); every other entity, and an entity outside the terrain's extent, at its interpolated y from the simulation; a projectile at its interpolated y. This departs from the simulation's y for the land and the seabed by design: the simulation's y is the height of the cell the entity stands in, while the drawn surface is the triangles of the mesh, which an entity's marker must not sink into or hover over. The test pins one entity of each layer on a sloped cell (docs/adr/0017-units-on-the-map.md).

**REN-071** `design` `test:supercom_marker_models_cases`
The binary MUST build four marker models by pure functions: a static one, a mobile one, a commander's, and a projectile's, in this order as models 0 to 3. Each lies in the plane y = 0, has the normal (0, 1, 0) at every vertex, is wound counter-clockwise seen from above, and passes the renderer's model check; they differ from one another; the mobile model, a triangle, and the commander's, a dart, point along the model's +x, their farthest vertex lying on the +x axis, so that a placement with yaw 0 shows the heading; the static one is a square and the projectile's a narrow rhombus. The test pins the vertex counts and the pointed vertex and checks the winding of every triangle (docs/adr/0017-units-on-the-map.md).

**REN-072** `design` `test:supercom_summary_line_text`
The run's summary line MUST be a pure function of the ticks run, the checkpoints matched, the checkpoints in the log, the count of frames at which the cap of REN-064 held ticks back, and the count of unit kinds drawn as markers because the run did not resolve them (REN-089), and read `replayed <ticks> ticks, <matched> of <total> checkpoints matched, <held> frames held back, <n> kinds drawn as markers`. The test pins the text for the committed log's end, 4600, 46 of 46, 0, and 0, for a run cut short with a count of 2, and for a count of 1, which is not given a singular (docs/adr/0017-units-on-the-map.md).

**REN-073** `design` `test:conformance_window_replay_prints_summary_then_frames`
When the step run at the last checkpoint's tick has run, stepping MUST stop and the window stay open on the final state, and the summary line of REN-072 MUST be written once to stdout, at that moment; a run that ends sooner, by the window's closing or by `--frames <n>`, MUST write it once at its end, as far as it got. With `--frames <n>` the run MUST present n frames, whether or not the log has finished, write the summary line, then `presented <n> frames`, and return the success code. A mismatch (REN-063) ends the run with the failure code and writes no summary line. The test runs the built binary on the committed log with `--frames 3` and a fresh cache directory, only when `SUPERCOM_REQUIRE_DISPLAY=1` is set, with the environment it inherits, so it is run only inside a nested compositor that shows nothing; without the flag it prints `SKIP: <reason>` and passes; with the flag and no install it fails; it waits at most two minutes and then kills the binary and fails. It expects one summary line, immediately before the line `presented 3 frames` which is the last (docs/adr/0017-units-on-the-map.md).

**REN-074** `design` `test:conformance_gpu_replay_start_draws_each_commander_in_its_army_colour`
At the start state of the committed log, with the instance list of REN-066 at the strategic view, the camera the renderer builds for the map's terrain at an aspect of 1, the frame drawn offscreen into a square target, the pixel each commander's drawn position projects to MUST hold its army's colour of REN-067, encoded with the target's transfer function, within 2 in 255 per channel, and the two commanders' pixels MUST differ. The test needs an adapter and the install, and expects the projected positions from the camera's own matrix, which is not under test (docs/adr/0017-units-on-the-map.md).

**REN-075** `design` `test:conformance_prepare_replay_checks_content_then_starts`
The steps before the window MUST succeed on the committed log and the installed game, returning the start state at tick 0 with the two commanders the log's two players start with, the replay with its 46 checkpoints, and the parsed map of 256 by 256 cells; and MUST refuse a log whose header's map digest differs, as an error value whose text names the content check and the start state, before the map file is parsed. The test imports into a fresh directory under the system's temporary directory (docs/adr/0017-units-on-the-map.md).

**REN-076** `design` `test:supercom_play_counts_held_frames_and_stops_at_the_last_checkpoint`
The one owner of the state MUST, each frame, take the ticks REN-064 says are due, in order, one step each, stop after the last checkpoint's tick, and count each frame at which the cap held ticks back, once, while stepping has not stopped; it reports the ticks run, the checkpoints matched, and that count. Ticks held back are stepped by later frames. The test steps a state of an empty world whose checkpoints it computed from its own copy of the state, over frames of 3 due and 2 held back, and expects the counts after each frame and after the last (docs/adr/0017-units-on-the-map.md).

## The unit meshes

The replay run draws each unit as the game's own lod0 mesh, lit, in its army's
colour, when the camera is near, and as its marker otherwise. The kinds of the
start state and the build orders are resolved before the window, through the
archive, blueprint, and mesh loaders only; the renderer learns nothing of what
a model depicts. A mesh is drawn untextured, without animation, from its stored
coordinates, with one forward yaw and the blueprint's scale. The record is
docs/adr/0017-units-on-the-map.md.

**REN-077** `game-data` `test:conformance_unit_meshes_sizes_and_scales`
For each of the five unit kinds the committed log names, ueb1103, ueb2101, uel0001, urb1101, and url0001, the installed game holds a lod0 mesh at `units/<ID>/<ID>_lod0.scm` and a blueprint at `units/<ID>/<ID>_unit.bp`, found without regard to case. The meshes hold, in that order, 1,660 vertices and 2,952 indices, 1,476 and 2,370, 5,807 and 10,458, 606 and 1,128, and 8,702 and 17,202, with no index at or past its vertex count, and the blueprints hold a `Display.UniformScale` of 0.06, 0.155, 0.105, 0.225, and 0.16. The test reads them through the archive, mesh, and blueprint loaders and needs the install.

**REN-078** `game-data` `test:conformance_unit_meshes_frame`
In the same five meshes +y is up: the lowest stored y is within 0.05 of 0 for ueb2101, uel0001, urb1101, and url0001, and is -10.284, within 0.0005, for ueb1103, whose stored vertices reach below its origin. A triangle is degenerate when its face normal, (b - a) x (c - a) computed in f64 from the stored f32 values for corners a, b, and c, is exactly zero; uel0001 holds 8 of 3,486 and the others none. Every other triangle is wound counter-clockwise seen from outside: its face normal has a positive dot product with the sum of its three corners' stored normals, a share of 1.0000 for each of the five. No stored normal of the five is off unit length by more than 1e-3, and none is shorter than 1e-6. The test reads the models the draw uses and needs the install.

**REN-079** `game-data` `test:conformance_unit_meshes_bones`
In the same five meshes a bone's place, under its stored inverse matrix transposed, is minus the transpose of R times t, where the transposed matrix, acting on column vectors, is [R t; 0 1]; a bone's name is matched with regard to case, `Muzzle` anywhere in it. The `Muzzle` bones of uel0001 (5), url0001 (3), and ueb2101 (1) lie at a mean z of 7.641, 4.024, and 3.156, within 0.0005, and ueb1103 and urb1101 have none. A vertex belongs to its first bone index; of the bones with ten or more vertices whose place lies farther from the origin than a tenth of the mesh's largest axis extent, the vertices' centroid is nearer the bone's place than the origin for 19 of 19 in uel0001, 18 of 18 in url0001, 9 of 11 in ueb1103, and 2 of 2 in ueb2101, and urb1101 has no such bone. The test reads the meshes through the loaders and needs the install.

**REN-080** `design` `test:supercom_unit_mesh_placement_cases`
Every unit kind MUST be drawn with one forward yaw, a quarter turn, the last argument of the renderer's placement function (REN-050), which takes a mesh's +z to the unit's heading: at heading 0 the mesh's +z lies along world +x. The stored coordinates are placed by that proper rotation, a positive scale that is the blueprint's `Display.UniformScale`, and the unit's drawn position, with no other fix-up: no mirror, axis swap, or reversal of a winding, and no rest transform. No bone, `rest_pose_inverse`, tangent, or binormal is read to draw. The stored positions are drawn as the rest pose, which is this project's reading of REN-079's counts and of the one bone reading under which every bone is consistent, not a statement of how the game draws; a vertex below the mesh's origin, as those of ueb1103 are, is drawn below the unit's position. The test sweeps six headings and expects the mesh's +z along (cos h, 0, sin h) times the scale, orthogonal columns each as long as the scale, a positive determinant, and the vertex 10.284 below the origin drawn 10.284 times the scale below the drawn position (docs/adr/0017-units-on-the-map.md).

**REN-081** `design` `test:supercom_unit_mesh_model_cases`
The model built from a mesh MUST be a pure function of the mesh: the stored positions, the stored normals taken from each vertex's `normal` as crates/scm names it after FMT-060, the indices widened to 32 bits, and nothing else. Two meshes that differ only in their bones, tangents, binormals, texture coordinates, and bone indices MUST give equal models. The test pins a four-vertex mesh whose last vertex lies 10.284 below the origin, an index of 65,535, and a mesh changed in everything the model does not read (docs/adr/0017-units-on-the-map.md).

**REN-082** `design` `test:supercom_unit_kinds_named_cases`
The unit kinds a run resolves MUST be a pure function of the start state and the replay: the blueprint of every entity of the start state and of every build order, mobile or factory, each in ASCII lower case, sorted and without repeats; no other order names a kind, and the binary states no faction's name. The test builds a state with the simulation's public constructor and spawn, holding entities of the blueprints uel0001, uel0001, and url0001, and a replay whose build orders name UEB1103, ueb1103, urb1101, and UEL0001, and expects ueb1103, uel0001, urb1101, url0001 in that order; it expects no kind for a state with no entity and a replay with no build order (docs/adr/0017-units-on-the-map.md).

**REN-083** `design` `test:supercom_unit_model_set_order`
The set of models the window uploads once before its loop MUST be the four markers of REN-071 followed by one model per resolved kind in the sorted order of the kinds' lower-case ids; a kind's index in the set is 4 plus its place among the kinds, and a kind not resolved has none. The test expects the indices and the models of two kinds, the absence of a third, and the markers alone for no kind (docs/adr/0017-units-on-the-map.md).

**REN-084** `design` `test:supercom_unit_kind_resolution_failures`
Each kind of REN-082 MUST be resolved before the window, through the archive, blueprint, and mesh loaders only, in these steps: the lod0 mesh read at `units/<id>/<id>_lod0.scm` and the blueprint read at `units/<id>/<id>_unit.bp`, by the lower-case id; the mesh parsed; the blueprint parsed; the scale taken from `Display.UniformScale`, which MUST be a number that is finite and positive as an `f32`; and the model of REN-081 passed to the renderer's check (REN-043, REN-086). A kind whose mesh or blueprint is missing or does not parse, whose scale is missing or unusable, or whose model the check refuses MUST end the run before the window with an error value that names the kind and the step: mesh read, blueprint read, mesh parse, blueprint parse, scale, or model check. Archives that do not mount are an error value of their own. The test expects each step's failure from a reader of fixed answers and a hand-built mesh, a scale absent, not a number, zero, negative, and past the range of `f32`, the scale refused before the model, the check's refusal naming the vertex, and a success that carries the scale and the model (docs/adr/0017-units-on-the-map.md).

**REN-085** `design` `test:conformance_prepare_replay_resolves_the_named_kinds`
The steps before the window MUST resolve the kinds of REN-084 after the map file and before returning. On the committed log and the installed game the kinds are, sorted, ueb1103, ueb2101, uel0001, urb1101, and url0001, the two commanders through the start state's entities and the other three through build orders, and the model set holds nine models, the five kinds at indices 4 to 8. A log with a build order for a kind the install lacks MUST end at that step, after every step before it, with an error value naming the kind and the mesh read. The test imports into a fresh directory under the system's temporary directory and needs the install (docs/adr/0017-units-on-the-map.md).

**REN-086** `design` `test:render_model_check_refuses_a_short_normal`
The renderer's check of a model MUST also refuse, after every refusal of REN-043, a normal whose squared length, computed in f64 from its f32 components, is below 1e-12, as an error value of its own naming the model's index in its set and the vertex. The installed meshes of UXB0008 and UXB0009 hold vertices whose normal is shorter than 1e-6 (FMT-060), so a run whose kinds include either ends at the model check of REN-084; mending installed normals is not done. A model's normals are otherwise taken as unit length and the shader renormalizes them. The test pins squared lengths of 0.98e-12 and 1.02e-12, a zero normal, and the order after an index out of range and a non-finite position (docs/adr/0017-units-on-the-map.md).

**REN-087** `design` `test:supercom_mesh_or_marker_rule_cases`
A unit MUST be drawn as its mesh when its kind was resolved and the camera's distance is at most 50 times the unit's own size (REN-068), and as its marker otherwise, never both: the rule is one pure function of the camera's distance and the unit's own size, true exactly when the size is positive and the distance is at or below the bound, so a distance or a size that is not a number draws the marker, and every entity is exactly one instance. The test pins literals on both sides of the threshold: at size 1 the distances 8, 49, and exactly 50 draw the mesh and 51 and 309.02 the marker; at size 3, 149 the mesh and 151 the marker; at size 8 the far end of the zoom, 309.02, draws the mesh; at size 1 a distance of 0 draws the mesh and one that is not a number the marker; at a distance of 0 a size of 0, of -1, or one that is not a number draws the marker; and over seven distances a list of three units holds three instances, each a lit mesh or a flat marker (docs/adr/0017-units-on-the-map.md).

**REN-088** `design` `test:supercom_scene_draws_meshes_and_markers`
The instance list MUST be that of REN-066 except for each unit that REN-087 draws as its mesh: that unit is one lit instance of the model of its kind (REN-083), placed as REN-080 says at its interpolated position and heading (REN-065, REN-070) with the blueprint's scale, in its army's colour (REN-067), dimmed by its build progress as its marker is. A unit whose kind was not resolved is its marker, as is every unit the rule draws as one; a projectile is always its marker. The test expects, at the near end of the zoom, the model index, the style, the colour, the position, and the scale of a mesh among markers and a projectile, the markers at the strategic view, the dimming of a unit half built, and one mesh at half a tick between kept and current values whose headings cross the wrap, from 6.2 to 0.1, at the midpoint of the two positions with its heading along the shorter arc (docs/adr/0017-units-on-the-map.md).

**REN-089** `design` `test:supercom_unnamed_kinds_are_markers_and_counted`
A unit whose kind the run did not resolve (REN-082), by the blueprint name its entity carries, MUST be drawn as its marker at every zoom, and the run MUST count each such kind once however often it is met and however many entities hold it, from the start state on; a projectile is not a kind and is never counted. The count is the last number of the summary line (REN-072). The stepping loop notes the kinds of the state when it is built and after every step, and makes the summary line from its own counts. The test expects the distinct names of a state of five entities, the marker of each at the near end, the count of a run noted twice, and the count for no resolved kind; it also expects that a stepping loop whose start state holds an entity of an unnamed kind counts 1 before any frame, that a kind met once and gone at the next noting stays counted, that a kind spawned between two steps is counted, and that the conformance loop over the committed log ends with 0 (docs/adr/0017-units-on-the-map.md).

**REN-090** `design` `test:conformance_gpu_close_zoom_draws_the_commander_mesh_lit_in_its_army_colour`
At the near end of the zoom, with the camera over the first commander of the committed log at its start state, the frame drawn offscreen into a square target MUST show that unit's mesh drawn lit in its army's colour: in a window of 96 pixels around the commander's projected position at least 150 pixels read as the army's colour times a light of 0.28 to 1.02, the lights differing by at least 0.15 across them and at least 100 of them under 0.95, while the other army's colour is read from fewer than one twentieth as many. The same frame with the commander drawn flat, with the commander removed, and with the commander in the other army's colour MUST each show fewer than 20 such pixels (fewer than 20 shaded pixels for the flat one), so that the test fails if the unit is drawn flat, not drawn, or in another army's colour. The test needs an adapter and the install (docs/adr/0017-units-on-the-map.md).

**REN-091** `design` `test:conformance_window_replay_at_ten_times_prints_one_summary`
The whole committed log at ten times the speed in one window, with 4,000 frames, which is enough to pass the last checkpoint, MUST write exactly one summary line, `replayed 4600 ticks, 46 of 46 checkpoints matched, <held> frames held back, 0 kinds drawn as markers`, followed by `presented 4000 frames`, and return the success code. The test is run only when `SUPERCOM_REQUIRE_DISPLAY=1` is set, inside a nested compositor that shows nothing; without the flag it prints `SKIP: <reason>` and passes; with the flag and no install it fails; it waits at most five minutes and then kills the binary and fails (docs/adr/0017-units-on-the-map.md).

## The textured terrain

The terrain is drawn with the map's own ground textures, blended by the
map's own masks as the original's terrain program blends them, where the
first slice drew a height ramp; the ramp stays as the fallback for the whole
terrain whenever a map's textures cannot be used. The renderer takes plain
pixel buffers, checks them against its device before it creates anything,
binds at most twelve sampled textures, and draws the terrain through a third
shader module, whose colour is the same rule as a pure function beside it.
The binary reads and decodes the textures and masks that the map's program
draws and hands them over. A rule taken from the original's terrain program
cites the archive effects.scd, its member effects/terrain.fx, and the lines,
and is stated here in the project's own words and numbers; nothing of the
member's text is kept. Left out, and not read: the terrain's normal maps,
decals, props, shadows, the sky, every water effect (the water stays the flat
plane of REN-022, with no tint by depth), a layer's own alpha as shine, and
the lighting of the original from a normal texture, a sun, or a shadow
lookup; the textured terrain is lit by the first slice's light alone, as
REN-101 has it. Units, markers, the camera, and the simulation are as they
were. The choices are recorded in docs/adr/0018-textures-on-the-map.md; the
tiling of a layer (REN-099) is provisional, and the first look, the project
owner's run of the binary on SCMP_012, SCMP_016, and the replay beside the
original, judges it, the mask's orientation (REN-098), the colour rule
(REN-101), and what a filtered lookup looks like.

**REN-092** `game-data` `test:conformance_ground_terrain_member_is_pinned`
The member effects/terrain.fx of the archive effects.scd, read in place through the archive reader, MUST be 47,280 bytes and hold 1,567 newline bytes (the line count a reader's tool prints is 1,568, which counts the empty line after the last), so that the line numbers the clauses of this section cite stay checkable. The test reads the member, counts, and prints nothing of it (docs/adr/0018-textures-on-the-map.md).

**REN-093** `game-data` `test:render_mask_weight_is_clamped_twice_the_value_less_one`
A mask channel's value m, from 0 to 1 (a stored byte over 255), MUST weigh clamp(2m - 1, 0, 1), so a stored byte of 127 or less weighs nothing, 128 weighs 1/255, 191 weighs 127/255, and 255 weighs one (effects.scd, effects/terrain.fx, lines 697, 728, and 729). The test pins those four bytes and 0 and 64.

**REN-094** `game-data` `test:render_blend_orders_layers_by_program_over_the_base`
The terrain's color MUST start from the layer of slot 0. Under the programs TTerrain and TTerrainGlow, the layers of slots 1 to 4 MUST then be blended over it in turn, each by linear interpolation by the weight (REN-093) of the first mask's first, second, third, and fourth channel, red, green, blue, and alpha as the decoder gives them (effects.scd, effects/terrain.fx, lines 708 to 712); under TTerrainXP the layers of slots 5 to 8 follow, weighed by the second mask's four channels in the same order (lines 742 to 751), and the first mask's channels weigh slots 1 to 4 as before. A slot the map leaves unnamed among those its program blends MUST be skipped, though its mask channel holds 255. Under TTerrain and TTerrainGlow the second mask MUST be ignored though its channels hold 255. The test works the blend on four layers and on eight, each layer's colour distinct and each weight distinct, so that a swapped order or channel gives another colour, and on the unnamed and ignored cases (the pixel test is REN-103).

**REN-095** `game-data` `test:render_top_layer_is_laid_on_by_its_own_alpha`
After the blend of REN-094, the layer of slot 9 MUST be laid over the color by linear interpolation by slot 9's own alpha, a stored byte over 255 (effects.scd, effects/terrain.fx, lines 713 and 751): an alpha of 0 leaves the color, 255 gives slot 9's color, and 100 gives 100/255 of the way. The alpha of every other layer is not read: in the original it feeds a shine term that is left out (lines 719 and 756). The test pins alphas of 0, 255, and 100.

**REN-096** `game-data` `test:conformance_ground_installed_unnamed_slot_masks_weigh_nothing`
Over the 20 installed maps whose program is TTerrainXP, the largest value of a second-mask channel whose slot (the channel's index plus 5, the slot it weighs) the map leaves unnamed MUST be 34 of 255, which is below 128 and so weighs nothing (REN-093); no installed map leaves a slot from 1 to 4 unnamed. The test decodes each such map's second mask in full through the decoder and measures the largest value, so that skipping an unnamed slot (REN-094) and weighing it are the same drawing on every installed map.

**REN-097** `design` `test:render_mask_coordinate_runs_zero_to_one_across_the_map`
A mask MUST be looked up across the whole map: the coordinate of the world point (x, z) on a map of extent (w, h) is (x / w, z / h), from 0 to 1 in each axis, clamped to 0 to 1, as the original hands on the world x and z as the coordinate (effects.scd, effects/terrain.fx, line 470) and scales every lookup by one whole-terrain factor (lines 111 and 697 to 703). The mask sampler clamps at the edges (lines 206 to 233). The test pins the corners, the centre, a quarter, and points past two edges (docs/adr/0018-textures-on-the-map.md).

**REN-098** `design` `test:conformance_gpu_mask_lookup_reads_north_west_first_and_clamps_at_the_edges`
A mask's first stored row MUST lie at the north edge of the map (z = 0) and its first column at the west edge (x = 0), with no flip of either: this orientation is this project's choice, since the installed files do not say it, and the first look judges it. The test draws a mask of 2 by 2 texels, each holding one channel, and expects the four quadrants' centres and a point a unit and a half inside each corner of the map to hold the layers of slots 1, 2, 3, and 4 in the north-west, north-east, south-west, and south-east, in Rgba8UnormSrgb, Bgra8UnormSrgb, and Rgba8Unorm (docs/adr/0018-textures-on-the-map.md).

**REN-099** `design` `test:render_layer_coordinate_repeats_by_world_over_scale`
PROVISIONAL, accepted by the project owner on 2026-11-26 and judged by the first look. One repeat of a layer MUST cover its stored scale in world units on each axis, so the coordinate of the world point (x, z) in a layer of scale s is (x / s, z / s) and a layer repeats (the map's width / s) times across the map and (its height / s) times down, for slots 0 to 9. The original's shader does not say how the engine derives its tiling factor from the stored scale (effects.scd, effects/terrain.fx, lines 698 to 703 and 731 to 740 multiply by a factor the engine sets); the rule is the one the FAF map editor's preview uses, at commit bac3e5f478e0293d04a4d18cf8dba95b57b5d49d, Assets/Scripts/Ozone SCMAP Code/ScmapEditor.cs lines 477, 493 to 500, and 514, and it fits the installed scales (4 for most layers, and 26.25 and 128 for slot 9 on SCMP_012 and SCMP_016, on maps 256 units wide). The first look asks whether the size of the ground's pattern matches the original. The test pins four points and four scales (docs/adr/0018-textures-on-the-map.md).

**REN-100** `design` `test:conformance_gpu_layer_repeats_by_world_over_scale`
The shader MUST repeat a layer as REN-099 states: a layer of 2 by 2 texels at the scale 32 on a map 64 wide shows each texel over 16 units and repeats twice across and down, and at the scale 64 shows each over 32 units and repeats once. The test expects, at the centres of 16 points in a 4 by 4 grid over the map, the texel each scale gives, in the three target formats (docs/adr/0018-textures-on-the-map.md).

**REN-101** `design` `test:render_textured_color_decodes_the_lit_stored_value`
Textures and masks MUST be uploaded as stored bytes in a format that applies no conversion on lookup, and the layers MUST be blended as stored numbers (a byte over 255), as the original's terrain program names no color-space conversion in either of its shaders (effects.scd, effects/terrain.fx, which does not mention sRGB). The blended color MUST be multiplied by the first slice's light, AMBIENT + (1 - AMBIENT) * max(n . L, 0) with the interpolated normal renormalized and AMBIENT and L as in REN-015, and the product MUST then be decoded by the sRGB transfer function (v / 12.92 at or below 0.04045, otherwise ((v + 0.055) / 1.055) to the power 2.4), the exact inverse of the encoding REN-021 applies for an sRGB target; the decoded value is the linear color the fragment returns. So on an sRGB target a pixel holds the stored-space result, a stored 128 lit at a factor of 1 is the pixel 128, and on a Unorm target the same texel is the pixel that 128/255 decodes to, 55. This is this project's choice, judged by the first look. The test pins the function at 0, 0.04045, 0.5, and 1, and the lit colors of a stored 128 at the factors 1, 0.86 (flat ground), and 0.3 (docs/adr/0018-textures-on-the-map.md).

**REN-102** `design` `test:conformance_gpu_textured_pixels_hold_the_stored_value_lit`
A textured terrain pixel MUST hold the color of REN-101, within 2 in 255 a channel: a stored 128 on a plane facing the light (a factor of 1) is 128 in Rgba8UnormSrgb and Bgra8UnormSrgb and 55 in Rgba8Unorm, and on flat ground (n . L = 0.8, a factor of 0.86) it is 110 and 40 (docs/adr/0018-textures-on-the-map.md).

**REN-103** `design` `test:conformance_gpu_textured_blend_follows_the_program_and_the_masks`
The shader MUST compute what REN-094 and REN-095 state. The test draws a plane facing the light under a first and a second mask of 4 by 4 texels with each channel value of 0, 127, 128, 191, and 255 used, and slot 9 with alphas of 0, 255, and 100 down its columns, and expects at the 16 texel centres the colors worked for eight layers, for four layers with the second mask ignored, and for eight with slots 2 and 7 unnamed, in the three target formats (docs/adr/0018-textures-on-the-map.md).

**REN-104** `design` `test:render_texture_checks_refuse_each_fault`
Before the renderer creates any texture, sampler, bind group, or pipeline for a set of terrain textures, pure functions MUST refuse it, as error values of the crate's Error and never a panic, for each of: a level whose bytes are not width times height times 4 (computed without overflow); a zero width or height; a side above the device's largest 2D texture side; a chain whose levels do not each halve the one before, rounding down and never below 1; a texture with no level; a scale that is not finite and positive; a second program whose second mask is missing; and a set that binds more sampled textures than the device allows a shader stage (the pass binds 12: ten layers and two masks, an unnamed layer and an unread mask bound to a one-texel stand-in). The refusals are checked in that order of item (slot 0, slots 1 to 8, slot 9, first mask, second mask), the binding count first. The test pins each refusal and a device limit of 11 against 12 (and the device-limit test is `render_texture_checks_refuse_a_device_with_too_few_sampled_textures`) (docs/adr/0018-textures-on-the-map.md).

**REN-105** `design` `test:conformance_gpu_a_refused_set_leaves_the_frame_and_the_fallback_matches_a_bare_renderer`
A set of terrain textures that the renderer refuses MUST leave the renderer exactly as it was: a renderer that drew the height ramp draws the same frame pixel for pixel, and one holding textures still holds them. A renderer given textures and then cleared MUST draw the frame of a renderer that was never given any. The test refuses a set for each fault of REN-104 that a device-independent buffer can show, through the renderer, and compares frames (docs/adr/0018-textures-on-the-map.md).

**REN-106** `design` `test:conformance_gpu_textures_leave_the_water_the_models_and_the_list_and_the_upload_leaves_the_textures`
Setting or clearing the terrain's textures MUST leave the mesh, the water plane, the model set, and the instance list as they were, and the terrain upload (REN-025) MUST leave the textures as they were. The terrain is drawn by the one draw function (REN-019), and the water, the models, and the instances are drawn as before. The test draws a scene with water and a flat instance before and after setting textures, expects the water's pixel and the instance's unchanged and the terrain's changed, and uploads again expecting the textures kept (docs/adr/0018-textures-on-the-map.md).

**REN-107** `design` `test:render_textured_shader_source_is_a_third_module_assembled_from_the_constants`
The textured terrain MUST be drawn through a third shader module with a source function of its own, the first module keeping its four entry points (REN-029) and the models' module staying the second (REN-054). The third module has the entry points vs_textured and fs_textured, binds the frame uniform of REN-029 and the textured terrain's uniform, two samplers, and twelve sampled textures, and writes the light direction and the ambient share from the crate's Rust constants. The terrain's vertex keeps its 24 bytes (REN-024); a layer's coordinate is computed from the world x and z. The test parses and validates the source with naga without an adapter and counts the entry points and the bound globals (docs/adr/0018-textures-on-the-map.md).

**REN-108** `design` `test:conformance_gpu_nearest_returns_one_texel_and_linear_blends_two`
Every pixel test of a texture MUST compare read-back pixels with a number worked from a clause, within 2 in 255 a channel, on the hardware adapter and on the software driver, with the nearest lookup at texel centres, which returns one stored texel; each has a form built from literal pixels that needs no installed game. The binary's lookups are filtered (REN-109); the two differ in the samplers' filtering alone, a choice handed over with the textures. Filtering, wrap, and the choice of a level are judged by a person's look, the first look, and by nothing else; the one measurement of the choice of a level is `conformance_gpu_filtered_lookup_chooses_a_smaller_level_where_texels_crowd`, which asserts that a layer of two levels seen with several texels to a pixel shows the second level and prints the pixels it read for a comparison of two adapters, which is reported and never asserted. This test expects a mask of two texels to read as one texel nearest and as the linear mixture of the two linear (docs/adr/0018-textures-on-the-map.md).

**REN-109** `game-data` `test:conformance_gpu_linear_lookups_clamp_masks_and_repeat_layers`
The binary's lookups MUST filter the masks linearly and clamp them at the edges, and filter the layers linearly with their levels and repeat them (effects.scd, effects/terrain.fx, lines 206 to 233 for the masks and 53 to 60 for the layers). The test draws, linearly filtered, a mask of two texels whose first holds 255 and expects a point a unit and a half inside the west edge to read that texel whole, not blended with the last, and a layer of two texels over one repeat and expects a point near the east edge to read the last texel blended with the first.

**REN-110** `game-data` `test:supercom_ground_program_names_choose_the_blend`
The blend MUST be chosen by the program the map names, matched exactly as the map stores it: TTerrain and TTerrainGlow blend slots 1 to 4 (REN-094), and TTerrainXP blends slots 1 to 8; any other name is the fallback's first case (REN-111). The installed maps name TTerrain on 39, TTerrainXP on 20, and TTerrainGlow on one, SCMP_023 (FMT-081; effects.scd, effects/terrain.fx, techniques at lines 852, 867, and 884). The glow map is drawn by TTerrain's rule; the two terms the original adds are left out, as this project's omission: layer two's lookup is shifted by an offset the vertex function supplies (lines 780 and 781), and the output's alpha carries a glow term from layer two's alpha and the second mask's channel (lines 782 and 804).

**REN-111** `design` `test:supercom_ground_decision_cases`
The terrain MUST be drawn by the height ramp, unchanged, with a counted notice, and never with a layer left out, in each of these cases: the program is none of the three; slot 0, slot 9, or a slot its program blends names a file that is missing or that the decoder refuses; a mask its program reads is missing or refused; and the renderer refuses the set (REN-104). The decision is one pure function of the program's name and what each read and decode returned. The readings the project builds: a map that leaves slot 0 or slot 9 unnamed falls back; a named slot its program blends whose stored scale is not finite and positive falls back, the reason naming the scale; in `view`, an install whose archives do not mount makes every file missing, so the run draws the ramp with its notice and does not end; a mask or a layer its program does not read is neither decoded nor a reason to fall back. A refusal never ends a run. The test pins each case, each reading, and the order of the notice's first item (REN-113) (docs/adr/0018-textures-on-the-map.md).

**REN-112** `design` `test:supercom_ground_gather_reports_each_reading_of_the_fallback`
Reading and decoding the files MUST report, for the cases of REN-111 that come from a file, the item, its path as the map stores it, and the reason: an unmounted install's every named file missing, a scale that is NaN, zero, negative, or infinite, a file the decoder refuses, an unnamed slot 0 or 9, a mask the map does not hold or the decoder refuses, and a second mask that the first program neither decodes nor needs. An unnamed slot from 1 to 8 gives a set with nothing at that slot (docs/adr/0018-textures-on-the-map.md).

**REN-113** `design` `test:supercom_ground_notice_names_the_item_the_file_the_reason_and_the_count`
The notice MUST be written once a run to stderr, name the first failing item in the order program, slot 0 to slot 9, first mask, second mask, with the file's path as the map stores it and the reason, and say how many items failed: `terrain textures not used, the height ramp is drawn: slot 3 (/maps/x/y.dds): cannot read: nope (2 failed)`; a mask has no file and its path is empty (docs/adr/0018-textures-on-the-map.md).

**REN-114** `design` `test:supercom_ground_gather_reads_only_what_the_program_draws`
Only what the map's program draws MUST be read and decoded: under TTerrain and TTerrainGlow the files of slots 0 to 4 and 9 in that order and the first mask, under TTerrainXP the ten files and both masks, and for a program that is none of the three nothing; each path is normalized as FMT-050 states (one leading slash stripped, each backslash a slash, lowercased) before the archive reader is asked. `supercom_ground_path_normalization_cases` pins the normalization (docs/adr/0018-textures-on-the-map.md).

**REN-115** `game-data` `test:conformance_ground_installed_reads_only_what_the_program_draws`
On SCMP_012 and on SCMP_016, whose program is TTerrain, exactly six texture files MUST be read through the archive reader and decoded to 7,340,024 bytes in all, and one mask of 128 by 128 decoded to 65,536 bytes; no other file is opened for the terrain; and the decision is the textured terrain with no notice. The test counts the reads and the bytes against those literals, and prints the time the mount and the read and decode take, on a first and a second run, which is reported and not asserted.

**REN-116** `design` `test:supercom_ground_hands_the_renderer_the_set_or_nothing`
For `view` and for `replay` alike, after the window and the renderer for its surface exist and before the terrain upload, one function of the binary MUST read and decode the textures, decide (REN-111), and hand the renderer the set or nothing; the window's build only calls it, so a test calls it without a window. A decision for the fallback leaves the renderer holding no textures (docs/adr/0018-textures-on-the-map.md).

**REN-117** `game-data` `test:conformance_gpu_ground_installed_scmp_012_draws_the_textured_terrain`
Through the binary's own path from the parsed map to the renderer, SCMP_012 and SCMP_016 MUST draw the textured terrain with no notice, and the frame at the strategic view, drawn into a square target of 512, MUST differ from the fallback's frame by more than 2 in 255 in some channel at more than half of the pixels of the middle 80 percent of its width and height; for SCMP_016 (`conformance_gpu_ground_installed_scmp_016_draws_the_textured_terrain_and_its_water`) the water's color MUST cover the fraction of the covered pixels that REN-031 states, within 0.03 of 0.095. The test needs an adapter and the install.
