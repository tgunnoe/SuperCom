# Render specification

This file specifies the first slice of the renderer: the part that needs no
graphics device. It covers the terrain grid built from a parsed map, the
water plane, the camera with its zoom range and clip planes, the height
normalization, and the shading function, all as pure functions over plain
`f32` arrays in the headless crate `supercom-render`. Nothing here draws or
opens a window. Its camera and shading are decisions of this project, recorded
in docs/adr/0016-first-light-renderer.md: a search of the installed data finds
no number for a world camera's field of view, clip planes, or zoom limits, so
no clause below says how the original behaves.

The frontier of this slice: the GPU pipeline's contract and the windowed mode
arrive with their code; terrain splatting, decals, shadows, water effects,
units, fog of war, strategic icons, and post-processing are later slices; and
the vertical reading of REN-001 awaits a comparison with the running original.

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
