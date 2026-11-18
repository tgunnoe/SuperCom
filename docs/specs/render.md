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
binary, tested without a window. Its fourth part is the models and instances
the same pipeline draws over the terrain and the water: plain data placed by
the caller, specified from the checks that refuse them to the pixels they
leave. Its fifth part is the replay run, the `replay` subcommand of the same
binary, which plays a recorded match in the window: the steps before the
window, the stepping and its comparison with the log's checkpoints, the
pacing and the interpolation, and what is drawn, every entity and projectile
as a flat marker over the terrain. Its camera and
shading are decisions of this project, recorded in
docs/adr/0016-first-light-renderer.md (the near end of the zoom range in
docs/adr/0017-units-on-the-map.md): a search of the installed data finds
no number for a world camera's field of view, clip planes, or zoom limits, so
no clause below says how the original behaves.

The frontier of this slice: the GPU pipeline's and the windowed mode's
contracts are specified below; the window loop itself, the code that turns the
window system's events into inputs and draws, is reached by no headless test
and is tried only when a display is available, so a person's run is its
evaluator (the record's section Known at ratification says what the run at
its ratification covered); a graphics device lost by the driver is reported by nothing (issue
74 of https://github.com/tgunnoe/SuperCom); the forge's runner has no graphics
adapter, so every `conformance_gpu_` test skips there and REN-029's parse and
validation are the only check the shader gets (issue 77 of
https://github.com/tgunnoe/SuperCom); two sentences, REN-035's order of the
fit check before the mesh and REN-040's last, are reached by no test (issue 78
of https://github.com/tgunnoe/SuperCom); the three installed maps of 4096
cells a side are refused and not drawn (issue 69); terrain splatting, decals,
shadows, water effects, fog of war, strategic icons, and
post-processing are later slices, and so are unit meshes: the replay run
draws markers; the frame the replay run's window draws is tried in a nested
compositor and by a person, while its stepping, its pacing, and its instance
list are pure and tested without a window; and the vertical reading of REN-001 awaits
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
A run of `view` MUST proceed in this order: the display (REN-033), the install, resolved as the bare run resolves it, the map file (REN-034), its parse, the window, the renderer for the window's surface, the grid fit check against that renderer's device, the terrain mesh, the water plane, the camera, the upload, and the loop. The fit check, REN-024, MUST run before the mesh is built, so a map the device cannot hold ends the run with its error and the mesh is never allocated. Each failure MUST end the run with its text on stderr and the failure code. The steps up to the window are one pure-in-effect function of the install directory and the folder; its test creates directories under the system's temporary directory, removes them, and expects the failure of each step: an unresolved install, a missing install, a folder that cannot be listed, none and two map files, and a file that does not parse. Its success on an installed map is exercised by a conformance test (docs/adr/0016-first-light-renderer.md).

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
projectile each frame as a flat marker over the terrain. Unit meshes are the
next slice: here everything is a marker. The binary reads the simulation
through its public surface only. The decisions are pure functions in the
binary, each tested without a window, except the clauses that name a display.
The record is docs/adr/0017-units-on-the-map.md.

**REN-057** `design` `test:supercom_prepare_replay_failures`
A run of `replay` MUST proceed in this order: the display (REN-058), the install, resolved as the bare run resolves it, the replay file and its parse, the map folder the replay's header names, held to REN-032's single-component rule, the cache directory (REN-059), the world, loaded from the install into that directory, the content check and the start state, which are the check the simulation's replay start makes of the header's content digests against the world and the skirmish it then starts, the map file and its parse as REN-035 has them for `view`, and then the window, what `view` builds, the marker models (REN-071), and the loop. A replay with no checkpoint MUST be refused after its parse, since the run would have no end. Every step before the window MUST be one function of the install directory, the file, and the cache variables, returning the parsed map, the replay, and the start state or an error value whose text names the failing step, and each failure MUST end the run with its text on stderr and the failure code. The test creates directories under the system's temporary directory, removes them, and expects the failure of each step that needs no installed game: an unresolved install, a missing install, an unreadable file, a file that does not parse, a replay with no checkpoint, a header map folder that is not a single component, an unresolved cache directory, a relative one, and a world that cannot be loaded from an install holding nothing (docs/adr/0017-units-on-the-map.md).

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
The run's summary line MUST be a pure function of the ticks run, the checkpoints matched, the checkpoints in the log, and the count of frames at which the cap of REN-064 held ticks back, and read `replayed <ticks> ticks, <matched> of <total> checkpoints matched, <held> frames held back`. The test pins the text for the committed log's end, 4600, 46 of 46, and 0, and for a run cut short (docs/adr/0017-units-on-the-map.md).

**REN-073** `design` `test:conformance_window_replay_prints_summary_then_frames`
When the step run at the last checkpoint's tick has run, stepping MUST stop and the window stay open on the final state, and the summary line of REN-072 MUST be written once to stdout, at that moment; a run that ends sooner, by the window's closing or by `--frames <n>`, MUST write it once at its end, as far as it got. With `--frames <n>` the run MUST present n frames, whether or not the log has finished, write the summary line, then `presented <n> frames`, and return the success code. A mismatch (REN-063) ends the run with the failure code and writes no summary line. The test runs the built binary on the committed log with `--frames 3` and a fresh cache directory, only when `SUPERCOM_REQUIRE_DISPLAY=1` is set, with the environment it inherits, so it is run only inside a nested compositor that shows nothing; without the flag it prints `SKIP: <reason>` and passes; with the flag and no install it fails; it waits at most two minutes and then kills the binary and fails. It expects one summary line, immediately before the line `presented 3 frames` which is the last (docs/adr/0017-units-on-the-map.md).

**REN-074** `design` `test:conformance_gpu_replay_start_draws_each_commander_in_its_army_colour`
At the start state of the committed log, with the instance list of REN-066 at the strategic view, the camera the renderer builds for the map's terrain at an aspect of 1, the frame drawn offscreen into a square target, the pixel each commander's drawn position projects to MUST hold its army's colour of REN-067, encoded with the target's transfer function, within 2 in 255 per channel, and the two commanders' pixels MUST differ. The test needs an adapter and the install, and expects the projected positions from the camera's own matrix, which is not under test (docs/adr/0017-units-on-the-map.md).

**REN-075** `design` `test:conformance_prepare_replay_checks_content_then_starts`
The steps before the window MUST succeed on the committed log and the installed game, returning the start state at tick 0 with the two commanders the log's two players start with, the replay with its 46 checkpoints, and the parsed map of 256 by 256 cells; and MUST refuse a log whose header's map digest differs, as an error value whose text names the content check and the start state, before the map file is parsed. The test imports into a fresh directory under the system's temporary directory (docs/adr/0017-units-on-the-map.md).

**REN-076** `design` `test:supercom_play_counts_held_frames_and_stops_at_the_last_checkpoint`
The one owner of the state MUST, each frame, take the ticks REN-064 says are due, in order, one step each, stop after the last checkpoint's tick, and count each frame at which the cap held ticks back, once, while stepping has not stopped; it reports the ticks run, the checkpoints matched, and that count. Ticks held back are stepped by later frames. The test steps a state of an empty world whose checkpoints it computed from its own copy of the state, over frames of 3 due and 2 held back, and expects the counts after each frame and after the last (docs/adr/0017-units-on-the-map.md).
