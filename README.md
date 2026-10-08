# SuperCom

A clean-room reimplementation of the Supreme Commander: Forged Alliance engine
in Rust, built from a reverse-engineered specification. The first milestone is
a skirmish match against a built-in AI on Linux/SteamOS, with strategic zoom,
factories, economy, weapons, pathfinding, and a basic HUD, reusing the art and
data of a locally installed copy of the game.

Status: pre-alpha. The specification is being written; the engine does not run
a match yet.

## You need your own copy of the game

SuperCom reads assets from a Forged Alliance installation at runtime. This
repository contains no game files, extracted assets, or game-derived scripts,
and never will. Point `SUPERCOM_FA_DIR` at your install; the Steam default
location is detected when the variable is unset.

## Development

- Environment: `nix-shell`, or `direnv allow` once. No flakes. The shell
  provides the Rust toolchain, cargo-component, wasmtime, nickel, and the
  formatters.
- `just gate` runs the full check: formatting, clippy, tests, and the license
  audit. `just` alone lists the recipes.
- Agents and contributors start at [AGENTS.md](AGENTS.md). Decisions live in
  `docs/adr/`; the specification index is
  [docs/specs/README.md](docs/specs/README.md).

## License

Not yet chosen. The repository is private.
