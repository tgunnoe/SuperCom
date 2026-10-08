# ADR-0003: Renderer on wgpu, winit, and egui; framework-free deterministic sim

**Status:** ACCEPTED

**Date:** 2026-10-07

---

## Context

The documented Supreme Commander frame is a bespoke DirectX 9 pipeline:
CPU-tessellated heightmap terrain with five-stratum texture splatting and
decals, light-space perspective shadow maps with blur, a mirrored-scene water
reflection plus refraction, per-unit albedo, normal, and specular/team-colour
maps, a 128x128 fog-of-war texture, strategic icons from a texture atlas, and
bloom. The first target is Linux/SteamOS; aarch64-linux and aarch64-darwin
must stay buildable. The simulation must run headless, deterministically, and
independently of rendering for lockstep multiplayer, replays, and tests.

## Decision

- Rendering uses `wgpu` (Vulkan on Linux, Metal on macOS) with hand-written
  passes that follow the documented pipeline. `winit` provides windowing and
  input; `egui` provides the HUD.
- The simulation is a framework-free `sim` crate with a fixed tick (target
  10 Hz as in the original, to be confirmed by the spec). It has no dependency
  on the renderer, builds and tests without a GPU or window, and exposes a
  command-stream interface.
- The application crate drives the sim and the renderer, interpolating
  between ticks for display.

## Consequences

### Positive

- Full control over a pipeline no general-purpose engine ships.
- The sim is testable and replayable with no graphics stack present.
- One codebase covers Vulkan and Metal.

### Negative

- More code than a framework would require: no asset pipeline or scene graph
  for free.
- `egui` is immediate-mode and suits the HUD only; richer UI needs its own
  design later.

---

## Alternatives Considered

| Alternative        | Reason Rejected                                                                                                                     |
| :----------------- | :---------------------------------------------------------------------------------------------------------------------------------- |
| Bevy               | Its render graph fights a bespoke pipeline; ECS scheduling must be isolated from the sim for determinism; heavy dependency surface. |
| Raw Vulkan via ash | No Metal path for macOS; a large unsafe surface for little gain over wgpu.                                                          |
| OpenGL via glow    | Deprecated on macOS; no compute; weaker tooling.                                                                                    |

---

## References

- AGENTS.md: R5, R6, I4.
- Graphics study:
  https://www.adriancourreges.com/blog/2015/06/23/supreme-commander-graphics-study/
- Related ADRs: ADR-0001 (plugin boundary).
