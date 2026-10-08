# ADR-0001: Plugin boundary is Wasm components over a WIT-defined engine API

**Status:** ACCEPTED

**Date:** 2026-10-07

---

## Context

The original engine is a C++ host (`MohoEngine.dll`) for a Lua 5.0 dialect
modified by Gas Powered Games (`continue`, `!=`, bitwise operators, `#`
comments). Unit behaviour, weapons, and AI live in 568 `*_script.lua` files
written against roughly 60 exposed engine classes. No pure-Rust Lua VM speaks
that dialect, so re-hosting the scripts would mean binding a C library (FFI)
or writing an interpreter, and it would carry a 2007 API design forward. The
head ruled against inheriting old design choices and against FFI, and asked
for a boundary that maximizes moddability while fitting Rust.

Two properties dominate the choice. Determinism: the sim must be identical on
every machine for lockstep multiplayer and replays, so a plugin must be unable
to introduce divergence. Formality: the mod API is a public contract, and a
typed, versioned interface fails at load time instead of mid-match.

## Decision

- Mods and first-party extensions are WebAssembly components executed by
  `wasmtime` (pure Rust, no FFI).
- The engine API is defined in WIT files under `wit/`. The host implements
  the imports; plugins implement the exports. The WIT world is the single,
  versioned contract.
- Sim-side plugin instances run with NaN canonicalization enabled and without
  clock, random, or filesystem imports; only the host-provided API is visible.
- A Rust SDK crate and a project template make writing a mod a `cargo new`
  away. Other Wasm-capable languages work without engine changes.
- v0 ships its behaviour (units, weapons, economy, AI) as built-in Rust shaped
  for this boundary. Which parts later move behind the WIT world is an open
  unknown (AGENTS.md U8).

## Consequences

### Positive

- One formal, machine-checked contract for every extension point.
- Determinism and sandboxing by construction, not by review.
- Plugins are JIT-compiled; heavy mods such as AI are not interpreter-bound.
- Modders can reuse the engine's own Rust types through the SDK.

### Negative

- Modders need a compile step; the SDK and template must stay polished.
- Existing SupCom and FAF Lua mods do not port directly.
- Host/guest data marshalling must be designed into the API (batching and
  handles rather than copies).

---

## Alternatives Considered

| Alternative                                                            | Reason Rejected                                                                                                                                       |
| :--------------------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Re-host the original Lua (FAF lua-lang via FFI, or a Rust interpreter) | Carries the 2007 API forward; FFI is excluded; the whole engine API would have to exist before anything moves on screen.                              |
| Starlark (starlark-rust)                                               | Deterministic and typed, but no coroutines and a second binding to maintain over the same API. Kept as a candidate for optional no-compile scripting. |
| Rune, Rhai, Koto                                                       | Dynamic typing; determinism enforced only by host discipline; weaker contract for a public mod API.                                                   |
| Piccolo (pure-Rust Lua 5.4)                                            | Buys Lua familiarity only; same contract weakness; still not the GPG dialect.                                                                         |
| Native Rust plugins via dynamic loading                                | Unstable ABI and `unsafe` loading; effectively FFI.                                                                                                   |

---

## References

- AGENTS.md: R3, I3, U8.
- Deterministic Wasm execution in wasmtime:
  https://docs.wasmtime.dev/examples-deterministic-wasm-execution.html
- Related ADRs: ADR-0002 (data model), ADR-0003 (render and sim stack).
