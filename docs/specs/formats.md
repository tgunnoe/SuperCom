# Formats specification

This file specifies the container and binary formats of the installed Forged Alliance data as SuperCom reads them: the `.scd` archive layer and its mount order, the data subset of the Lua dialect that blueprints and map scripts use, `.scm` meshes and `.sca` animations, DDS texture headers, the `.scmap` binary map, and the sound-bank inventory. Each clause names one evaluator. A clause whose evaluator is a test or a script is machine-verified: the tests are the loader crates' conformance and synthetic tests, cited by full name; the scripts read only `SUPERCOM_FA_DIR`, read archive members in memory, print counts, constants, and identifiers, print `SKIP:` and succeed when the install is absent, and fail under `SUPERCOM_REQUIRE_GAME=1` without it. A clause whose evaluator is a pinned source is cited, not run. Counts are those of the installed Forged Alliance data. Statements about the original engine rest on a pinned citation or a script- or test-verified data fact; a rule of this project is a `design` clause and says so. Nothing here reproduces game or reference content: only identifiers, counts, constants, and byte layouts appear.

The data model built on these formats is specified in `docs/specs/data-model.md`.

## Archive layer and mount order

**FMT-001** `game-data` `test:conformance_install_mounts_every_gamedata_archive`
The ⟨Vfs: Vfs⟩ MUST mount every `*.scd` archive directly under the install's `gamedata/` directory and serve from it any file that no other archive holds. The installed game has 17 such archives, of which 14 hold a file that no other archive holds.

**FMT-002** `game-data` `test:conformance_vfs_enumerates_each_visible_file_once`
The ⟨Vfs: Vfs⟩ MUST enumerate each visible path exactly once, paths being compared without regard to case. The 17 installed archives hold 23,015 file entries between them (directory entries excluded) and 23,011 visible paths, the difference being the 4 paths that two archives hold.

**FMT-003** `game-data` `test:conformance_sampled_files_read_stored_with_their_listed_size`
An archive member MUST be read as stored bytes whose length equals the size its archive lists. Every hundredth visible file of the installed game, at least 200 files, reads as a stored member of exactly its listed size.

**FMT-004** `design` `test:archive_read_reports_the_compression_method`
An ⟨Archive: Archive⟩ MUST NOT decompress: reading a member whose ZIP compression method is not 0 returns an error that carries the method (method 8 in the test), and reading a stored member of the same archive still succeeds. The crate's own documentation states the same scope.

**FMT-005** `game-data` `script:tools/spec_eval/formats/archive.py FMT-005`
The installed file `bin/SupComDataPath.lua` holds four top-level mount calls, in lines 20 to 23, in this order: two mounts of the contents of directories under the user's personal folder, then one mount of the `gamedata` archive glob `*.scd` at the root path `/`, then one mount of the directory one level above `InitFileDir` at `/`. The glob occurs once in the file. These four calls order mount points; they do not state in what order the archives matched by the glob are mounted, nor which mount supplies a path that two mounts hold.

**FMT-006** `design` `test:vfs_mounts_in_byte_order_and_the_first_mounted_wins`
The ⟨Vfs: Vfs⟩ of `crates/scd` is the one authority for mount order in this project, because FMT-005 does not fix the order within the glob: it MUST mount archives in ascending byte order of their file names (plain byte comparison, no locale, no case folding, so `B.scd` precedes `a.scd`), and the first-mounted copy of a shared path MUST win, later archives still contributing the paths no earlier archive holds. The blueprint importer does not use the Vfs and opens six named archives itself; the gap is specified in `docs/specs/data-model.md` (DAT-016).

**FMT-007** `game-data` `script:tools/spec_eval/formats/archive.py FMT-007`
Exactly four paths are held by two archives: `lua/sim/projectile.lua`, `lua/sim/unit.lua`, `lua/sim/prop.lua`, and `lua/simutils.lua`, each in `lua.scd` and in `mohodata.scd`, the two copies of each differing in content. Which of the two copies the original engine reads is not established by any source this specification cites; the project's rule is FMT-006, and the original's choice is an open frontier whose signpost is a pinned engine citation or an observation of the running game.

**FMT-008** `derived` `test:conformance_first_mounted_archive_wins_shared_paths`
Following from FMT-006 and FMT-007, for each of the 4 shared paths the ⟨Vfs: Vfs⟩ MUST return the bytes of the copy in the earlier-mounted archive, and that copy differs from the later one.

**FMT-009** `game-data` `test:conformance_lookup_ignores_case`
A path lookup in the ⟨Vfs: Vfs⟩ or in an ⟨Archive: Archive⟩ MUST ignore case: the upper-cased and lower-cased forms of a stored member path return the bytes of the stored form.

## Data subset of the Lua dialect

**FMT-010** `game-data` `test:conformance_bp_every_unit_parses`
The reader of the dialect's data subset MUST parse every one of the 568 `_unit.bp` members of `units.scd` as a table wrapped in a `UnitBlueprint` constructor call.

**FMT-011** `game-data` `test:conformance_bp_every_projectile_parses`
The reader MUST parse every one of the 365 `_proj.bp` members (287 in `projectiles.scd`, 78 in `effects.scd`) as a table under a `ProjectileBlueprint` constructor.

**FMT-012** `game-data` `test:conformance_bp_every_prop_parses`
The reader MUST parse every one of the 336 `_prop.bp` members (334 in `env.scd`, 1 in `meshes.scd`, 1 in `props.scd`) as a table under a `PropBlueprint` constructor.

**FMT-013** `game-data` `test:conformance_bp_unit_keys_present`
Across the 568 parsed unit bodies the keys `Categories`, `Defense`, `Economy`, `General`, and `Physics` MUST be present in every body, `Intel` in exactly 395, and `Weapon` in exactly 247.

**FMT-014** `game-data` `test:conformance_bp_every_save_parses`
The script entry point of the reader MUST parse every one of the 61 `_save.lua` files in the folders of `maps/`, each a sequence of top-level `name = value` assignments.

**FMT-015** `game-data` `test:conformance_bp_every_scenario_parses`
The script entry point MUST parse every one of the 61 `_scenario.lua` files in the folders of `maps/`.

**FMT-016** `game-data` `test:conformance_bp_markers_extracted`
Marker extraction over the 61 save files MUST yield exactly 19,363 markers, each with a non-empty name, a non-empty kind, and a finite position. A marker is an entry of the `Markers` table under `MasterChain` in the `Scenario` assignment that is a table holding a `type` call with a string argument and a `position` call with three numbers; an entry lacking either is skipped.

**FMT-017** `game-data` `script:tools/spec_eval/formats/dialect.py FMT-017`
The reader's v0 scope is the 568 unit, 365 projectile, and 336 prop blueprints and the 122 map Lua files (61 `_save.lua` and 61 `_scenario.lua`); it is never claimed for every `.bp`. The installed game also holds 2,437 `EmitterBlueprint` and 184 `TrailEmitterBlueprint` files, 2,621 in all, none of which the data-subset reader used by the evaluators accepts; they are outside v0 scope. The 103 `BeamBlueprint` and 72 `MeshBlueprint` files parse with the reader and are counted by the duplicate-key census of FMT-026 (1,444 files, 1,269 of them units, projectiles, and props), but no clause of this document or of the data-model document claims them as v0 inputs beyond that census. One further `.bp` file opens with no constructor identifier of the kinds counted here.

**FMT-018** `design` `test:brace_form_calls_carry_the_table_as_their_one_argument`
The reader MUST keep a constructor call whose argument is a brace-delimited table as a call value that carries the table as its one argument, without evaluating it; the call identifier is preserved as written.

**FMT-019** `design` `test:parenthesised_calls_are_kept_structurally`
The reader MUST keep a constructor call with parenthesised arguments as a call value holding its arguments in order, including the zero-argument form, without evaluating it.

**FMT-020** `design` `test:string_escapes_resolve_in_both_quote_styles`
The reader MUST resolve the escapes `\n`, `\\`, `\"`, and `\'` inside both single-quoted and double-quoted strings, and MUST accept an apostrophe unescaped inside a double-quoted string.

**FMT-021** `design` `test:unknown_escape_is_a_syntax_error`
The reader MUST reject a string holding an escape it does not define as a syntax error, never keep the character.

**FMT-022** `design` `test:hash_and_dash_comments_are_skipped`
The reader MUST skip `#` and `--` comments to the end of the line and MUST NOT treat those characters as comment starts inside a quoted string.

**FMT-023** `design` `test:function_is_unsupported_and_named`
A file that needs a construct outside the data subset, such as a function definition, MUST be reported as unsupported with the line and the name of the construct, never partially read.

**FMT-024** `design` `test:parse_bytes_decodes_iso_8859_1`
The reader's byte entry point MUST decode its input as ISO-8859-1, one character per byte, so that every byte sequence decodes.

**FMT-025** `design` `script:tools/spec_eval/formats/guards.py FMT-025`
The `Encoding` variant of the blueprint reader's error type is reserved: no code path returns it, because the decoding of FMT-024 is total. The documentation of the byte entry point still lists the variant among the errors it may return; the code governs and the lag is tracked in the repository's issue tracker (issue 23).

**FMT-026** `design` `script:tools/spec_eval/formats/dialect.py FMT-026`
A duplicate named key (a key repeated in one table constructor) MUST be rejected as malformed input, never resolved to the first or last value. The installed data never exercises the rule: across the 1,444 blueprint files that open with a unit, projectile, prop, beam, or mesh constructor, no table repeats a named key. The gap: the reader's `Value::get` in `crates/bp/src/lib.rs` and the Nickel renderer (`nickel_decl` and `nickel_body` in `crates/data/src/lib.rs`) keep the first value today; the repository's issue 25 tracks the implementation. No claim is made here about how Lua resolves a repeated key.

## Meshes and animations

**FMT-027** `reference` `source:https://raw.githubusercontent.com/Exotic-Retard/SupCom_Import_Export_Blender/d5cd48b6f0c3/supcom-importer.py#L328`
A `.scm` mesh is little-endian: a 48-byte header (the magic `MODL` and eleven `u32`, line 328 of the pinned importer), then tables placed by file offsets: 108-byte bone records (line 208), 68-byte vertex records (line 281, sixteen `f32` and four `u8`), and triangle indices (line 412). The importer carries no licence and is read for understanding only.

**FMT-028** `game-data` `test:conformance_scm_every_mesh_parses`
The mesh parser MUST accept all 1,938 version-5 `.scm` members of the install. The one other `.scm` member is a version-3 file of 1,905 bytes, and the parser MUST reject it with an unsupported-version error that names version 3.

**FMT-029** `game-data` `test:conformance_scm_tables_match_the_header`
For every installed version-5 mesh the bone list MUST have `total_bone_count` records and the vertex and index lists the header's counts, every bone MUST have a non-empty name and a parent below the bone count, and the installed meshes hold 10,752 bones and 1,812,633 vertices in all.

**FMT-030** `design` `test:scm_mesh_parent_minus_one_is_none_and_an_index_is_some`
A bone record's parent field of -1 MUST read as no parent (a root) and any other stored value as a parent index.

**FMT-031** `design` `test:scm_mesh_bone_name_of_1024_bytes_parses`
The mesh parser MUST accept a bone name of exactly 1,024 bytes and keep its length. The bound exists so that many bone records pointing at one string cost memory linear in the file.

**FMT-032** `design` `test:scm_mesh_bone_name_of_1025_bytes_is_malformed`
The mesh parser MUST reject a bone name longer than 1,024 bytes as malformed.

**FMT-033** `design` `test:scm_anim_bone_name_of_1025_bytes_is_malformed`
The animation parser MUST reject a bone name longer than 1,024 bytes as malformed, and MUST accept one of exactly 1,024 bytes (`scm_anim_bone_name_of_1024_bytes_parses`).

**FMT-034** `design` `source:crates/scm/src/lib.rs`
The parsers return mesh triangle indices, vertex bone indices, and animation bone links as stored, unchecked against the vertex count, the bone count, or the bone-name list: the mesh parser checks only that the index count is a multiple of three and that a bone's parent index names a bone. A consumer MUST validate every such index before using it to index a table. The repository's issue 18 tracks tightening the parsers.

**FMT-035** `game-data` `test:conformance_scm_indices_in_range`
In every installed version-5 mesh every triangle index is below the vertex count, every vertex bone index is below the bone count, and the index count is a multiple of three. The parsers do not enforce this (FMT-034); the test pins that the installed data satisfies it.

**FMT-060** `game-data` `test:conformance_scm_vertex_vectors_order`
A version-5 `.scm` vertex stores, after its position, its surface normal, then its tangent, then its binormal. This order was measured, not read from the importer FMT-027 cites, which names the vectors position, tangent, normal, binormal at its line 281. For a triangle with corners a, b, and c the face normal is (b - a) x (c - a), computed in f64 from the stored f32 values; the triangle is degenerate when that normal is exactly zero, and it agrees with a vector when their dot product is above zero. A triangle whose three corner vectors sum to exactly zero has no direction: it is left out of that vector's mean cosine and counts as not agreeing. In each of the install's 1,938 version-5 meshes, the mean absolute cosine between each non-degenerate triangle's face normal and the sum of its three corners' second stored vectors exceeds the same mean for the third and for the fourth. Over the non-degenerate triangles, 1,839 of those meshes agree with that sum on a share of at least 0.99; the other 99 fall below it, 74 under `env/`, 5 under `projectiles/`, and 20 under `units/`, the lowest share being 0.8800, in `env/Evergreen/Props/Trees/Pine06_s2_lod1.scm`. Nine vertices have a second vector shorter than 1e-6: 3 in `meshes/game/map-border_squ_cybran_a_lod0.scm`, 2 in `units/UXB0008/UXB0008_LOD0.scm`, and 4 in `units/UXB0009/UXB0009_LOD0.scm`.

**FMT-036** `reference` `source:https://raw.githubusercontent.com/Exotic-Retard/SupCom_Import_Export_Blender/d5cd48b6f0c3/supcom-importer.py#L625`
A `.sca` animation is little-endian: a 36-byte header (the magic `ANIM` and eight 4-byte words, line 625 of the pinned importer), then the bone names, the bone links, and the animation data, which holds one root pose (line 662) before the frames; a frame is a key time and a flags word (line 496) followed by one pose of seven `f32` per bone (line 502).

**FMT-037** `game-data` `test:conformance_sca_every_animation_parses`
The animation parser MUST accept all 475 installed `.sca` members, which are all version 5, and each parsed animation MUST hold as many frames as its header's frame count and one pose per bone name in every frame.

**FMT-038** `design` `test:sca_animation_frame_size_contradicting_bone_count_is_malformed`
The animation parser MUST reject as malformed a header whose frame size is not `8 + 28 * bone_count` bytes.

**FMT-039** `game-data` `test:conformance_sca_fields_match_the_header_and_poses`
In every installed animation the frame count, duration, and bone count match the header, there is one bone link per bone and each is -1 or a bone index, key times never decrease and never exceed the duration, and every pose rotation is a unit quaternion; the install holds 112,623 frames and 1,777,774 poses.

## Texture headers

**FMT-040** `game-data` `test:conformance_dds_every_header_parses`
The DDS header parser MUST accept all 14,275 installed `.dds` members, reading the height at offset 12 and the width at offset 16 of the 128-byte header, and MUST report the byte count after the header as the data length.

**FMT-041** `game-data` `test:conformance_dds_format_census`
The parser MUST classify the installed textures as 11,211 DXT5, 1,250 DXT3, 1,242 DXT1, and 572 uncompressed (533 at 32 bits, 36 at 24 bits, 3 at 8 bits), and none as an unclassified FOURCC.

**FMT-042** `game-data` `test:conformance_dds_mip_count_follows_the_flag`
The parser MUST report the header's mip-count field only when the `DDSD_MIPMAPCOUNT` flag is set, and 1 otherwise: 6,721 installed textures set the flag, always with a count above 1, and 7,554 do not.

**FMT-043** `game-data` `test:conformance_dds_data_fits`
For each of the 13,703 installed block-compressed textures the byte size of the full mip chain, by 4 by 4 blocks of 8 bytes (DXT1) or 16 bytes (DXT3 and DXT5), is at most the data length, and equals it in 13,620 of them.

**FMT-044** `design` `source:crates/dds/src/lib.rs`
The header parser returns the width, height, and mip count as stored and does not check them against the data length or against each other; a consumer MUST validate the dimensions and the mip count before sizing a buffer or a mip chain from them. The repository's issue 18 tracks tightening the parser.

**FMT-045** `design` `script:tools/spec_eval/formats/guards.py FMT-045`
The `UnsupportedVersion` variant of the DDS error type is reserved: DDS has no version field, no code path returns the variant, and it exists only to keep the error shape the three binary loaders share.

**FMT-046** `design` `test:dds_dwsize_other_than_124_is_malformed`
The DDS header parser MUST reject as malformed a header whose `dwSize` is not 124, and MUST report a buffer shorter than 128 bytes as truncated, not as malformed.

## Maps

**FMT-047** `game-data` `test:conformance_scmap_every_map_parses`
The map parser MUST accept all 60 installed `.scmap` files, which carry the signature `0x1a70614d`, major version 2, and inner version 60. The heightmap grid equals the world size and holds `(width + 1) * (height + 1)` signed 16-bit samples.

**FMT-048** `game-data` `test:conformance_scmap_size_class_census`
The installed maps fall into six size classes by heightmap grid: 10 at 256 by 256, 18 at 512 by 512, 22 at 1024 by 1024, 1 at 2048 by 1024, 6 at 2048 by 2048, and 3 at 4096 by 4096.

**FMT-049** `game-data` `test:conformance_scmap_preview_is_dds`
The preview image embedded in each map MUST be a whole DDS file, accepted by the DDS header parser of FMT-040, whose data is the rest of the embedded blob.

**FMT-050** `game-data` `test:conformance_scmap_strata_resolve`
Every map has ten terrain albedo strata slots (600 in all), of which 170 are unused (empty path) and 430 are named; every named texture path, after normalization, is a path of the ⟨Vfs: Vfs⟩. Normalization strips one leading slash, maps each backslash to a slash, and lowercases the path.

**FMT-051** `design` `test:scmap_strata_keep_their_slots_paths_and_scales`
The map parser MUST return each stratum's texture path as stored, with a leading slash, backslashes, and mixed case intact, and keep its slot and tiling scale; normalization is the consumer's, applied as in FMT-050, and is not done by the crate.

**FMT-052** `design` `source:crates/scmap/src/lib.rs`
A map whose major version is not 2 MUST be rejected as malformed; the unsupported-version error is reserved for the inner version and names it.

**FMT-053** `design` `test:scmap_inner_version_other_than_56_or_60_is_unsupported`
The map parser MUST reject an inner version other than 56 and 60 with an unsupported-version error naming the version found (the test uses 0, 55, 57, and 61), and MUST read a version-56 map as a version-60 one apart from the one extra `f32` that only version 60 carries (`scmap_version_56_map_parses_like_version_60`).

**FMT-054** `reference` `source:https://raw.githubusercontent.com/FAForever/Neroxis-Map-Generator/10b130836034/shared/src/main/java/com/faforever/neroxis/importer/SCMapImporter.java#L82`
The map layout this project reads follows the pinned reference reader: the embedded preview (lines 78 and 446-453), the heightmap of two cell counts, a scale, signed 16-bit samples and one trailing byte (lines 82-90), the water block that follows the lighting (read at line 109): its presence byte compared with 1 (line 354) and the surface, deep, and abyss elevations (lines 355-357 of `readWaterSettings`, which starts at line 353), and the ten strata, each a null-terminated path and a scale, after a version-dependent skip (lines 126-136).

**FMT-055** `design` `source:crates/scmap/src/lib.rs`
The map parser walks the sections between the heightmap and the strata (shader and sky paths, cube maps, lighting, water and its wave generators, minimap words) with checked sizes and returns only the water: whether the map has any (the presence byte equal to 1) and the surface, deep, and abyss elevations, read from the first three floats of the water block, with none of the water's other terms (`scmap_water_block_reads_presence_and_elevations`); decals, props, and the sections after the strata are not read, and the `_scenario.lua` and `_save.lua` files beside a map are read by the dialect reader of FMT-014 and FMT-015, not by the map parser.

**FMT-056** `design` `test:conformance_scmap_every_map_parses`
The map conformance suite decodes about 106 million heightmap samples in all, the sum of `(width + 1) * (height + 1)` over the size classes of FMT-048. It is slow in a debug build (about 6 seconds on one development machine); it runs with the other conformance tests and MUST NOT be sampled to save time.

**FMT-059** `game-data` `test:conformance_scmap_water_census`
Of the 60 installed maps, 43 carry water (presence byte 1), with surface elevations from 0 to 30, the deep elevation at or below the surface and the abyss elevation at or below the deep on every one; the other 17 carry none (presence byte 0), the folders `SCMP_003`, `SCMP_004`, `SCMP_006`, `SCMP_007`, `SCMP_012`, `SCMP_013`, `SCMP_015`, `SCMP_017`, `SCMP_018`, `SCMP_022`, `SCMP_023`, `SCMP_026`, `SCMP_031`, `SCMP_036`, `SCMP_038`, `X1MP_001`, and `X1MP_002`.

## Sound banks

**FMT-057** `game-data` `script:tools/spec_eval/formats/banks.py FMT-057`
Counted recursively, `sounds/` holds 89 `.xwb` wave banks, each opening with the magic `WBND` and format version 43, and 90 `.xsb` sound banks; `sounds/` has 160 top-level entries, and the voice banks sit in per-language folders below it. The inventory reads only the magic and the version word of each wave bank.

**FMT-058** `design` `source:AGENTS.md`
The project MUST NOT choose an audio codec or decoder from this inventory: which codecs the 89 wave banks use is not inventoried (U6 in AGENTS.md), and no loader for `.xwb` or `.xsb` is specified until a codec inventory across the banks exists.
