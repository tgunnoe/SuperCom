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
  blueprints, 1148 meshes, 63 maps). Signpost: a v0 feature needs an asset
  type with no loader.
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
  files into that model and caches it in the user data directory. Grounding:
  docs/adr/0002-data-model-nickel.md. Signpost: full-set evaluation exceeds
  the load budget even with the cache, or imports fail through
  `nickel-lang-core`.
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
  predicate `/forge`. Signpost: a feature reaches `main` without a PR.

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
  non-test library paths. Grounding: predicate `rust` skill. Signpost:
  `clippy::unwrap_used` warnings in a library crate.
- **I7 Game path is configuration.** The install directory comes from
  `SUPERCOM_FA_DIR` or the config file, never a hard-coded path. Grounding:
  R1, R6. Signpost: a Steam path literal outside the default-resolution
  function.

## Unknowns

Each item: statement · grounding (why unresolved) · signpost (what resolves it).

- **U1 Float strategy for lockstep.** Fixed-point sim versus IEEE floats with
  deterministic transcendental functions. Grounding: transcendental and FMA
  behaviour differs across x86_64 and aarch64. Signpost: a cross-architecture
  replay comparison in CI.
- **U2 Nickel load cost.** Evaluating ~580 blueprints with contracts.
  Grounding: no benchmark yet. Signpost: measured cold and cached load time
  against a budget set in the data-model spec.
- **U3 Engine/Lua split.** Which behaviours the original C++ engine owned
  versus its Lua layer, which fixes the Rust sim surface. Grounding: the FAF
  engine docs list about 60 exposed classes; the split is not yet inventoried.
  Signpost: the spec's engine-API inventory is complete.
- **U4 Project license.** Grounding: not yet chosen; repo is private.
  Signpost: the head decides before the repo goes public.
- **U5 Map format coverage.** `.scmap` inner versions 56 and 60 are documented;
  installed maps are untested. Signpost: the loader parses all 63 installed
  maps.
- **U6 Audio codecs.** Which codecs the installed XACT banks use. Signpost: a
  codec inventory across `sounds/*.xwb`.
- **U7 macOS.** wgpu (Metal) and wasmtime are known good on aarch64-darwin;
  the rest of the dependency set is untested. Signpost: a green macOS build.
- **U8 Built-in versus plugin.** Which v0 behaviours (AI in particular) ship
  as first-party Wasm plugins rather than built-in Rust. Signpost: an ADR
  after the spec campaign.

## Operational entrypoint

- Read first: this file, docs/specs/README.md, docs/adr/.
- Environment: `direnv allow` or `nix-shell` (no flakes). If `IN_NIX_SHELL`
  is set, every tool below is already on PATH; do not nest `nix-shell`.
- Commands: `just check`, `just test`, `just clippy`, `just fmt`, `just gate`
  (the CI gate: format check, clippy with `-D warnings`, tests, license audit).
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

Core (planned; only `crates/supercom` exists today — WIP):

- `crates/supercom` — binary: configuration, wiring, launches a skirmish.
- `crates/scd` — `.scd` archive virtual filesystem and mount order.
- `crates/data` — Nickel contracts and the typed blueprint model;
  `crates/bp-import` — `.bp` to Nickel importer and cache.
- `crates/scm`, `crates/scmap`, `crates/xact` — mesh/animation, map, and
  sound-bank loaders.
- `crates/sim` — deterministic simulation (economy, units, orders, weapons,
  projectiles, pathfinding, damage); `crates/ai` — built-in skirmish AI.
- `crates/plugin` — wasmtime host and plugin SDK; `wit/` — the engine API.
- `crates/render` — wgpu renderer; `crates/ui` — winit/egui HUD, input,
  camera and strategic zoom.

Context-sensitive: `docs/specs/` (authority), `docs/adr/` (decisions),
`docs/models/` (formal models), `tools/` (one-off importers and probes; none
yet).

## Spec Pointers

- docs/specs/README.md — the reverse-engineering spec index.
- docs/adr/ — decisions 0001–0004.
- Graphics pipeline study:
  https://www.adriancourreges.com/blog/2015/06/23/supreme-commander-graphics-study/
- Engine Lua API surface (read for understanding only, see I1):
  https://github.com/FAForever/fa/tree/develop/engine
- Lua dialect reference: https://github.com/FAForever/lua-lang
- Map format reference implementation:
  https://github.com/FAForever/Neroxis-Map-Generator/blob/develop/shared/src/main/java/com/faforever/neroxis/importer/SCMapImporter.java
- Mesh/animation layout: https://github.com/Exotic-Retard/SupCom_Import_Export_Blender
- Reconstructed engine header: https://github.com/FAForever/FA-Binary-Patches/blob/master/include/moho.h
