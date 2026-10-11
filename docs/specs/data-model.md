# Data-model specification

This file specifies the blueprint data model of SuperCom: the Nickel contracts and the category vocabulary, the typed catalog the Rust crates expose, the importer that builds it from the installed game, its cache, the load budget, and the v0 pipeline as an interim toward the single-schema design of `docs/adr/0002-data-model-nickel.md`. The formats the importer reads are specified in `docs/specs/formats.md`. Each clause names one evaluator. A clause whose evaluator is a test or a script is machine-verified: the tests are the existing conformance and synthetic tests of `crates/data`, `crates/bp-import`, and `crates/bp`, cited by full name; the scripts read only `SUPERCOM_FA_DIR` or only this repository, print counts, constants, and identifiers, print `SKIP:` and succeed when an install they need is absent, and fail under `SUPERCOM_REQUIRE_GAME=1` without it. A clause whose evaluator is a pinned source is cited, not run. Where a clause states a target the code does not meet, the gap is named in the clause and cites the code that shows it.

## Contracts and vocabulary

**DAT-001** `design` `test:valid_unit_passes_its_contract`
The unit contract (`crates/data/nickel/unit.ncl`) MUST require `Categories`, an `Economy` record holding `BuildCostEnergy` and `BuildCostMass`, a `Physics` record holding `MotionType`, and a `Defense` record holding `Health`, `MaxHealth`, and `ArmorType`; every other v0 field is optional, and the records are open, so a field the contract does not name cannot change its verdict. The projectile and prop contracts (`projectile.ncl`, `prop.ncl`) require nothing and declare only an optional `Categories` list of strings.

**DAT-002** `design` `test:unknown_unit_category_violates_the_contract`
The unit contract MUST reject a unit whose `Categories` holds a string outside the category vocabulary, and the failure MUST surface as a contract error.

**DAT-003** `game-data` `test:conformance_data_category_vocab`
The category vocabulary (`crates/data/nickel/categories.ncl`) MUST equal the set of distinct category strings across the 568 installed unit blueprints: 152 tags, 151 upper-case identifiers and the mixed-case `Cybran`.

**DAT-004** `game-data` `test:conformance_data_every_unit_validates`
All 568 installed unit bodies MUST pass the unit contract and project into the typed ⟨Unit: Unit⟩ model.

## Optional fields and lists

**DAT-005** `design` `test:minimal_unit_projects_with_absent_fields_as_none`
A scalar field that the installed data omits MUST be an `Option` in the typed model and read as `None` when absent, and a list it omits MUST be an empty `Vec`; the model MUST NOT invent an engine default. The optional scalars are `BuildTime`, `BuildRate`, the production and storage fields, `MaxSpeed`, `TurnRate`, `RegenRate`, `VisionRadius`, `RadarRadius`, `RateOfFire`, `MaxRadius`, and the weapon's `Label` and `ProjectileId`; the optional blocks are `Intel` and the `Weapon` list.

**DAT-006** `game-data` `script:tools/spec_eval/data-model/absence.py DAT-006`
The optional fields of DAT-005 are optional exactly where the installed data omits them: `BuildTime` is absent in 16 of 568 units, `VisionRadius` in 4 of 395 `Intel` blocks, `RateOfFire` in 91 and `MaxRadius` in 86 of 494 weapon entries (the 91 holding 90 and the 86 holding all 86 with `WeaponCategory` `Death`), and `Categories` in 90 of 365 projectiles and 37 of 336 props.

**DAT-007** `design` `test:projectile_and_prop_contracts_allow_absent_categories`
A projectile or prop body without `Categories` MUST validate, and MUST project with an empty category list.

**DAT-008** `design` `test:agreement_on_a_nil_build_time`
The Rust presence and type checks that `validate` applies before the contract MUST read a field holding `nil` as an absent field, so a `nil` optional field is accepted and a `nil` required field is missing; the renderer writes an accepted body's `nil` optional field as absent, so the contract accepts that body as the Rust checks did. What the contract alone would decide on a `nil` required field is the single-schema node's question (DAT-029).

**DAT-009** `design` `test:agreement_on_an_infinite_build_time`
The Rust checks that `validate` applies before the contract MUST refuse a number that is not finite (Lua reads `1e999` as infinity) and MUST name the field in the error; the contract never receives such a body (the renderer would write the value as `null`), and what the contract alone would decide on it is the single-schema node's question (DAT-029).

**DAT-010** `design` `test:agreement_on_a_named_key_weapon_table`
The Rust checks that `validate` applies before the contract MUST reject a table with a named key where an array is declared (the `Weapon` and `Categories` lists), and both the Rust checks and the contract MUST accept an empty or positional `Intel` table as a record with no declared field; what the contract alone would decide on the rejected shapes is the single-schema node's question (DAT-029).

**DAT-011** `design` `test:conformance_data_every_unit_validates`
Whatever the projection accepts, the contract MUST accept, modulo the vocabulary: a body the Rust checks accept is accepted by the contract unless its only fault is a category outside the vocabulary, which only the contract rejects. This accept direction is what is evaluated: on the eight synthetic bodies of `crates/data/tests/agreement.rs` (`agreement_on_an_empty_intel_table`, `agreement_on_a_positional_intel_table`, `agreement_on_a_nil_build_time`, `agreement_on_a_nil_vision_radius`, `agreement_on_an_infinite_build_time`, `agreement_on_a_named_key_weapon_table`, `agreement_on_a_named_key_categories_table`, and `a_fully_populated_unit_validates`) and, by the evaluator, on the 568 installed unit bodies, all accepted by both. The reject direction (the Rust checks reject every body the contract rejects, modulo the vocabulary) holds by composition, because `validate` applies the projection before the contract, and is not evaluated separately. A generative test over bodies the projection accepts is an open frontier; its signpost is a property test in `crates/data/tests` asserting that `validate` accepts every generated body the projection accepts, or the single-schema node of DAT-029.

**DAT-012** `design` `source:crates/data/src/lib.rs`
The data crate SHOULD offer one public way to obtain a ⟨Unit: Unit⟩ from a parsed body, a single validated entrypoint that applies the contract and then projects. Today a consumer calls `validate` and then `unit_from_value`, two calls to be made in that order, and the public projections accept a few shapes that the contract rejects, through the vocabulary alone. Adopting the entrypoint makes the projections private, which changes the existing tests that call them directly; that change is an open frontier, with the single-schema work of DAT-029 as its signpost.

## Catalog and ids

**DAT-013** `game-data` `test:conformance_import_catalog_counts`
The importer MUST return a catalog of 568 units, 365 projectiles, and 336 props. Units come from `units.scd` alone; projectiles and props come from every archive that holds them.

**DAT-014** `design` `test:conformance_import_ids_unique_per_family`
A unit's id MUST be its lower-cased file stem without `_unit`; a projectile's or prop's id MUST be its lower-cased member path with a leading slash. Each family's ids MUST be pairwise distinct and the family listed in ascending id order. The path form exists because the bare stem is not unique across archives and directories.

**DAT-015** `game-data` `test:conformance_import_projectile_references_resolve`
Every installed unit weapon's `ProjectileId`, lower-cased, MUST equal the id of a projectile in the catalog.

**DAT-016** `design` `script:tools/spec_eval/data-model/importer.py DAT-016`
The importer opens six named archives directly, in the order of its own list (`units.scd`, `projectiles.scd`, `effects.scd`, `env.scd`, `meshes.scd`, `props.scd`), and sorts its results by id; it does not use the ⟨Vfs: Vfs⟩, whose rule is FMT-006 of `docs/specs/formats.md`. Three orders therefore exist today: the Vfs mount order, the importer's list, and the id order of the results. The Vfs is the one authority for mount order; the direct opening is a named gap, and it is not observable today because none of the 4 shared paths of FMT-007 is a blueprint.

## Cache

**DAT-017** `game-data` `test:conformance_import_cache_round_trip`
Loading a catalog twice through the cache MUST leave a cache file after the first call, and the second call MUST return a catalog equal to the first.

**DAT-018** `game-data` `test:conformance_import_cache_key_is_stable`
The cache key MUST be non-empty and the same on two calls over an unchanged install.

**DAT-019** `design` `test:conformance_import_cache_format_is_two`
The cache key MUST lead with the format version (currently `v2`), so a cache written under another format version never matches and is a miss.

**DAT-020** `design` `test:float_bits::round_trip_sweep_of_bit_patterns_keeps_bits`
The cache's encoding MUST round-trip every finite `f64` bit-exactly (the test sweeps 100,000 bit patterns from a seeded generator; the literal set is `float_bits::round_trip_of_literals_keeps_bits`), because the catalog is a candidate input to the lockstep initial state (DAT-024).

**DAT-021** `design` `test:load_without_a_cache_or_install_fails_and_writes_nothing`
A failed import MUST NOT leave a cache file. A successful write goes to a temporary name and is renamed into place; an unreadable or unparsable cache file counts as a miss.

**DAT-022** `design` `script:tools/spec_eval/data-model/cache.py DAT-022`
The cache key of the current implementation is the manual `CACHE_FORMAT` constant followed by each of the six archives' name, size, and modification time. It is blind to an in-place replacement of an archive that keeps its size and modification time, to a change of the contract text, and to a change of the importer's reading of a member that does not bump the constant. The evaluator checks that the key reads size and modification time and that the importer source holds no digest of content.

**DAT-023** `design` `source:crates/bp-import/src/lib.rs`
The cache key SHOULD instead cover the content of each archive the import reads, the text of the contracts, and the importer's version, so that no constant needs a manual bump; DAT-022 names the current implementation's gap.

**DAT-024** `design` `source:docs/specs/determinism.md`
A content digest of the ⟨Catalog: Catalog⟩ MUST exist before a match can be replayed from the catalog: it is computed over a canonical serialization with a fixed field order, ids in ascending order, and `f64` values as bit patterns, as `docs/specs/determinism.md` requires of state (DET-009), and it takes its place among the installed-data digests of a replay header (DET-007). No such digest exists yet; the cache key of DAT-022 is not one.

**DAT-025** `design` `source:AGENTS.md`
The cache is regenerable and MUST NOT be committed. It belongs in the user cache directory, not the user data directory that requirement R4 of AGENTS.md and `docs/adr/0002-data-model-nickel.md` name, because a cache the importer can rebuild is not user data.

## Load budget

**DAT-026** `design` `script:tools/spec_eval/data-model/cache.py DAT-026`
The load budget MUST be 10 seconds for a cold import of the v0 set and 1 second for a load from the cache. The repository's only source of the figures is `crates/bp-import/src/bin/bench_import.rs`, whose constants `COLD_BUDGET` and `CACHED_BUDGET` hold them; AGENTS.md (requirement R4 and unknown U2) names the budget without numbers. The evaluator checks the constants and the four lines the benchmark prints; it does not time the import.

**DAT-027** `design` `source:crates/bp-import/src/bin/bench_import.rs`
The release benchmark `bench_import` MUST print four lines, `cold_import`, `cache_write`, `cached_load`, and `mount_share`, and exit 1 when the cold import exceeds the cold budget or the cached load exceeds the cached budget. The measured figures are per-machine evidence that lives with the change that landed the benchmark, not in this clause. `mount_share` times the whole-install mount through the ⟨Vfs: Vfs⟩, which the import never performs (the benchmark's own header says so), so it bounds a mount cost the import does not pay and is not part of either budget.

## Interim pipeline and overlays

**DAT-028** `design` `test:conformance_data_every_unit_validates`
The v0 pipeline is an interim toward `docs/adr/0002-data-model-nickel.md`: the importer parses a blueprint, checks presence and types in Rust, applies the Nickel contract only for the vocabulary check, and caches the Rust projection. The interim holds while every installed body that the projection accepts the contract also accepts, which the evaluator checks over all 568 units (DAT-004, DAT-011).

**DAT-029** `design` `source:docs/adr/0002-data-model-nickel.md`
The target, which the interim of DAT-028 does not yet meet, is one schema: Nickel records carrying the contracts, the contract check running after overlays are merged, and the serialized result cached. The post-merge guarantee does not hold for overlays under the interim, because the Rust checks run before any merge. The single-schema node that replaces the interim is a frontier; its signpost is an overlay merged over a base record and then checked by the contract in the importer.

**DAT-030** `design` `source:docs/adr/0002-data-model-nickel.md`
The category vocabulary MUST be the 152-tag base of DAT-003 plus the categories that a mod overlay declares, and the contract MUST be assembled from them per load, never fixed at compile time. Today the vocabulary is spliced into the contract once at compile time (`CATEGORIES_NCL` in `crates/data/src/lib.rs`), so an overlay cannot extend it; that is a named gap.

**DAT-031** `design` `test:projection_ignores_fields_outside_the_v0_set`
A field that neither the contract nor the projection names MUST NOT change the verdict or the projected value. Because the records are open, a misspelt optional field is such a field: it is silently absent, not an error. A consumer that needs misspelling to fail MUST be given a stricter contract; none exists in v0.
