# ADR-0005: The project is licensed GPL-3.0-or-later

**Status:** ACCEPTED

**Date:** 2026-10-07

---

## Context

SuperCom is a clean-room reimplementation that reads a proprietary game's
assets at runtime and will carry a public mod API. The head wants the engine
itself to stay free software: derived engines and distributed modifications
must ship their source. The license was an open unknown (formerly AGENTS.md
U4) until the head ruled on it.

## Decision

- The engine and every workspace crate are licensed under the GNU General
  Public License, version 3 or (at your option) any later version
  (`GPL-3.0-or-later`). The full text is in `LICENSE`.
- Dependencies must carry GPL-compatible licenses. The allowlist in
  `deny.toml` is permissive-only for now (MIT, Apache-2.0, BSD, ISC, Zlib,
  CC0, Unicode); widening it to copyleft dependencies is a deliberate edit,
  not a side effect of adding a crate.
- Game assets are never covered: they stay in the player's own installation
  and are never distributed with the engine (AGENTS.md I1).
- Plugins run as separate Wasm components across a defined interface. Whether
  that boundary makes a plugin a separate work is a question for the plugin
  API spec to state explicitly, not something this ADR settles.

## Consequences

### Positive

- Forks and redistributed builds must publish their changes.
- The permissive dependency allowlist keeps the engine GPL-compliant by
  construction, checked by `just gate`.

### Negative

- Some contributors and studios avoid copyleft code; proprietary forks are
  excluded by design.
- The plugin-API spec owes modders a clear statement on licensing of their
  own components.

---

## Alternatives Considered

| Alternative       | Reason Rejected                                                                                  |
| :---------------- | :----------------------------------------------------------------------------------------------- |
| MIT or Apache-2.0 | Permits closed forks of the engine, which the head does not want.                                |
| GPL-3.0-only      | Forecloses adopting a future GPL revision; the head said "GPLv3" without that restriction.       |
| AGPL-3.0          | Network-use clause adds nothing for a locally run game and deters hosting of community services. |

---

## Amendment 1 (2026-10-08)

The head admitted LGPL-3.0 (needed by `nickel-lang-core` through `malachite`)
and MPL-2.0 to the dependency allowlist. Both are compatible with
GPL-3.0-or-later; both are weak copyleft, so they do not change the
project's own licence. The allowlist in `deny.toml` is the record.

---

## References

- `LICENSE` at the repository root; `Cargo.toml` workspace `license` field.
- AGENTS.md: I1, I8.
- Related ADRs: ADR-0001 (plugin boundary).
