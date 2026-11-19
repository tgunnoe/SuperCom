# AGENTS.md — SuperCom

SuperCom is a clean-room reimplementation of the Supreme Commander: Forged
Alliance engine in Rust, built from a reverse-engineered specification. It
loads art, blueprints, maps, and sounds from a locally installed copy of the
game at runtime. Governed by predicate
(https://github.com/nrdxp/predicate, pinned at 4e533f0): read its `rules.md`
and `ambient.md` before non-trivial work; the Commit Gate is installed in
`.git/hooks`.

Maturity: `molten` (pre-1.0). Refactor and cut freely; design over stability.

## Goal

A playable skirmish match against a built-in AI on Linux/SteamOS, using the
installed game's assets: strategic zoom, factories building units, economy,
weapons and projectiles, unit movement and pathfinding, a basic HUD. Core
skirmish and multiplayer play only; menus, campaign, movies, tutorials, and
DirectX are out of scope. Every subsystem is specified before it is built:
a spec clause with a named evaluator precedes the code that satisfies it.

## Requirements

Each item: statement · grounding · signpost (what defeats it).

- **R1 Asset reuse.** All v0 art, blueprints, maps, and sounds come from the
  installed Forged Alliance (primary) or Supreme Commander (reference) data,
  read in place. Grounding: `~/.local/share/Steam/steamapps/common/Supreme
Commander Forged Alliance/` (`gamedata/*.scd` store-only zips: 568 unit
  blueprints; 1,939 meshes across the archives, 1,148 of them in `units.scd`;
  60 maps in 61 folders; counts from `tools/survey/`). Signpost: a v0 feature
  needs an asset type with no loader.
- **R2 Deterministic simulation.** The sim is a pure function of (initial
  state, command stream, tick); identical on every supported platform.
  Grounding: lockstep networking and replays are command streams
  (docs/specs/README.md, determinism domain). Signpost: two runs of one
  command log diverge, on one machine or across x86_64 and aarch64.
- **R3 Plugin boundary.** Mods and first-party extensions are Wasm components
  against a WIT-defined engine API hosted by wasmtime. Grounding:
  docs/adr/0001-plugin-boundary-wasm-components.md. Signpost: a v0 behaviour
  that the WIT world cannot express.
- **R4 Canonical data in Nickel.** Blueprints, balance, and mod overlays are
  Nickel records validated by contracts; an importer converts installed `.bp`
  files into that model and caches it in the user cache directory. Grounding:
  docs/adr/0002-data-model-nickel.md. The installed armour table is read
  from its Lua by the ingest crate in this version, outside the Nickel model
  (the head's ruling of 2026-10-18); its overlay reopens when mod overlays
  land. A unit's mesh scale is read from the installed blueprint by the binary
  in this version, outside the Nickel model (the head's ruling of 2026-11-17);
  it reopens when unit textures or mod overlays land. Signpost: full-set
  evaluation exceeds the load budget even with the cache, or imports fail
  through `nickel-lang-core`.
- **R5 Rendering on wgpu.** Terrain splatting, decals, LiSPSM shadows, water
  reflection/refraction, team-colour unit shading, fog of war, strategic
  icons, bloom. Grounding: docs/adr/0003-render-and-sim-stack.md and the
  graphics study linked under Spec Pointers. Signpost: a required technique
  cannot be expressed on wgpu's feature set.
- **R6 Portable.** x86_64-linux first; aarch64-linux and aarch64-darwin stay
  buildable. Grounding: rust-toolchain.toml targets. Signpost: a dependency
  without an aarch64 or darwin port.
- **R7 Spec first.** docs/specs/ is the authority; each clause names its
  evaluator and traces to a test. Grounding: predicate `/spec`. Signpost:
  code lands for a subsystem with no spec clause.
- **R8 Reviewable.** Major features land as pull requests on
  https://github.com/tgunnoe/SuperCom for the head's review. Grounding:
  predicate `/forge`. Signpost: a feature reaches `master` without a PR.

## Invariants

Each item: statement · grounding · signpost (what would violate it).

- **I1 No proprietary content in the repo.** No game files, extracted assets,
  or copied game/FAF Lua, ever. Grounding: the game is proprietary; the FAF
  `fa` repository carries no license. Signpost: a binary asset, `.bp`,
  `.scm`, `.scmap`, or game-derived `.lua` in `git ls-files`.
- **I2 Sim hermeticity.** Sim code never reads the clock, OS randomness, the
  filesystem, or the iteration order of unordered maps. Grounding: R2.
  Signpost: the replay-divergence test fails.
- **I3 Sandboxed plugins.** Sim-side plugins run under wasmtime with NaN
  canonicalization and no clock, random, or filesystem imports. Grounding:
  R2, R3. Signpost: a plugin can desync a match or escape the sandbox.
- **I4 Headless core.** The sim and asset loaders build and test without a
  GPU or window. Grounding: R2, R7. Signpost: `cargo test` needs a display.
- **I5 Gated commits.** Every commit passes the predicate Commit Gate; the
  tree is formatted with `treefmt` before commit. Grounding: predicate
  `rules.md` §3. Signpost: a commit landed with `--no-verify`.
- **I6 Library code returns `Result`.** No `unwrap`, `expect`, or `panic!` in
  non-test library paths. Test support may panic by contract when a required
  fixture is absent, under `#[expect(clippy::panic, reason = ...)]` (the test
  kit's `require_game`). Grounding: predicate `rust` skill. Signpost:
  `clippy::unwrap_used` warnings in a library crate.
- **I7 Game path is configuration.** The install directory comes from
  `SUPERCOM_FA_DIR` or the config file, never a hard-coded path. Grounding:
  R1, R6. Signpost: a Steam path literal outside the default-resolution
  function.
- **I8 GPL-compatible dependencies.** The project is GPL-3.0-or-later; every
  dependency carries a GPL-compatible license from the allowlist in
  `deny.toml`. Grounding: LICENSE and
  docs/adr/0005-license-gpl-3.0-or-later.md. Signpost: the license audit in
  `just gate` fails on a new dependency.

## Unknowns

Each item: statement · grounding (why unresolved) · signpost (what resolves it).

- **U1 Cross-architecture float determinism.** ADR-0006 chose binary64 with
  the exactly specified operations, `libm` transcendentals, and NaN
  canonicalization at the serialization boundary; the DET-024 replay
  comparison has passed twice by hand on x86_64 and aarch64 (the recorded
  runs in docs/adr/0006-float-strategy.md), not yet in CI, and aarch64-darwin
  is unmeasured. Grounding: the same record's aarch64 section. Signpost: the
  comparison running in CI on both architectures and once on aarch64-darwin
  (the head's ruling of 2026-10-22).
- **U6 Audio codecs.** Which codecs the 89 installed XACT wave banks use
  (docs/specs/formats.md inventories the banks only). Signpost: a codec
  inventory across `sounds/**/*.xwb` (issue 28).
- **U7 macOS.** wgpu (Metal) and wasmtime are known good on aarch64-darwin;
  the rest of the dependency set is untested (issue 27). Signpost: a green
  macOS build.
- **U9 Shadowed Lua paths.** Four `lua/` files (`lua/sim/Unit.lua`,
  `lua/sim/Projectile.lua`, `lua/sim/Prop.lua`, `lua/SimUtils.lua`) exist in
  both `lua.scd` and `mohodata.scd` with different contents; the archive layer
  reads the `lua.scd` copy (docs/specs/formats.md, the mount-order clause),
  which the sim specification's citations match, but which copy the original
  engine reads is unestablished and the sim campaign's citations rest on it.
  The `lua.scd` copies are the larger ones (the unit script is 142,533 bytes
  against 3,757 in `mohodata.scd`, the projectile script 17,416 against 614,
  and `SimUtils.lua` 7,301 against 4,396, which is no stub), consistent with
  the mount order. Signpost: a pinned engine citation of the mount precedence, or
  an observation of the running original reading one of the four.
- **U10 Observing the original.** Two observations of the running original
  are recorded under `tools/spec_eval/sim/observed/` (SIM-086: the back-off's
  direction; SIM-087: the defeat kill takes unfinished sites, about 20
  seconds after the defeat — neither with tick-level numbers); no clause is
  in the specification lint's `observed` class, and every other behavioural
  rule rests on readable Lua or on a recorded choice. Signpost: the first
  record with tick-level numbers that a clause is tested against.
- **U11 Facing and mirroring against the original.** Whether a drawn unit is the
  mirror image of the original cannot be told from the installed files
  (docs/adr/0017-units-on-the-map.md), and structures face east here where the
  head reports south in the original (issue 93). Grounding: the head's look of
  2026-11-18, made without the original beside the window. Signpost: a
  side-by-side observation recorded as U10's are.
- **U8 Built-in versus plugin.** Which v0 behaviours (AI in particular) ship
  as first-party Wasm plugins rather than built-in Rust. Signpost: an ADR
  after the spec campaign.

## Operational entrypoint

- Read first: this file, docs/specs/README.md, docs/adr/.
- Environment: `direnv allow` or `nix-shell` (no flakes). If `IN_NIX_SHELL`
  is set, every tool below is already on PATH; do not nest `nix-shell`.
- Commands: `just check`, `just test`, `just clippy`, `just fmt`, `just lint`
  (the specification lint: self-test, check, run), `just scan` (the clean-room
  content scan), `just gate` (the CI gate: format check, lint, scan, clippy
  with `-D warnings`, tests, license audit).
- Game data: `SUPERCOM_FA_DIR` (defaults to the Steam install path).
- Predicate arsenal by moment: `rust` for all code; `spec` and `form` for the
  reverse-engineered specification; `prior-art` before any format loader or
  algorithm; `robust-testing` with every implementation; `security-audit` for
  the plugin host and file parsers; `boundary` then `campaign` for any
  multi-workstream initiative. Model tiers for dispatch:
  docs/adr/0004-agent-tier-routing.md.
- Ledger: `.ledger/` (separate repo, git-ignored) is the flight recorder;
  `.scratch/` is the campaign working set, never committed.

## Structure

Core (planned; `crates/supercom`, `crates/scd`, `crates/testkit`,
`crates/plugin` with `wit/`, `crates/detbench`, `crates/scm`, `crates/dds`,
`crates/scmap`, `crates/bp`, `crates/data`, `crates/bp-import`, `crates/sim`,
`crates/sim-ingest`, `crates/replay-harness`, and `crates/render` exist today —
WIP):

- `crates/supercom` — binary: configuration, wiring; today `view` draws an
  installed map and `replay` plays a recorded match (units as markers, and as
  the game's meshes up close); launching a skirmish is planned.
- `crates/scd` — `.scd` archive virtual filesystem and mount order.
- `crates/bp` — the data subset of the blueprint Lua dialect, parsed to a
  value tree; `crates/data` — Nickel contracts and the typed blueprint model;
  `crates/bp-import` — the importer and its cache.
- `crates/scm`, `crates/dds`, `crates/scmap`, `crates/xact` — mesh/animation,
  texture-header, map, and sound-bank loaders.
- `crates/sim` — deterministic simulation (economy, build, movement and
  planning, weapons, projectiles, damage, vision, victory; the replay format
  and the digest chain); `crates/sim-ingest` — the installed catalog and a map
  loaded into the sim's world; `crates/replay-harness` — the committed
  skirmish log, the digest program, and the cross-architecture comparison,
  never shipped; `crates/ai` — built-in skirmish AI.
- `crates/plugin` — wasmtime host and plugin SDK; `wit/` — the engine API.
- `crates/detbench` — determinism evidence: golden vectors for the float
  strategy and the on-demand benchmark; never shipped.
- `crates/render` — wgpu renderer, with the camera and its strategic zoom; the
  window, the event loop, and the camera's input are in `crates/supercom`;
  `crates/ui` — egui HUD and command input.

Context-sensitive: `docs/specs/` (authority), `docs/adr/` (decisions),
`docs/models/` (formal models), `tools/` (the specification lint and the
clean-room scan with their red fixtures, `spec_eval/` evaluator scripts per
specification domain, `survey/` census scripts over the installed game).

## Spec Pointers

- docs/specs/README.md — the reverse-engineering spec index.
- docs/adr/ — decisions 0001–0017.
- Graphics pipeline study:
  https://www.adriancourreges.com/blog/2015/06/23/supreme-commander-graphics-study/
- Engine Lua API surface (read for understanding only, see I1):
  https://github.com/FAForever/fa/tree/develop/engine
- Lua dialect reference: https://github.com/FAForever/lua-lang
- Map format reference implementation:
  https://github.com/FAForever/Neroxis-Map-Generator/blob/develop/shared/src/main/java/com/faforever/neroxis/importer/SCMapImporter.java
- Mesh/animation layout: https://github.com/Exotic-Retard/SupCom_Import_Export_Blender
- Reconstructed engine header: https://github.com/FAForever/FA-Binary-Patches/blob/master/include/moho.h
