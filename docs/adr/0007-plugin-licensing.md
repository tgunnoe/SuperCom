# ADR-0007: Plugins are derivative works licensed GPL-3.0-or-later or compatible

**Status:** ACCEPTED (ruled by the head on 2026-10-09)

**Date:** 2026-10-09

## Context

ADR-0005 licenses the engine and every workspace crate under GPL-3.0-or-later,
leaves open whether the plugin boundary makes a plugin a separate work, and
says the plugin API specification owes modders a clear statement. ADR-0001
chose Wasm components over a WIT engine API for moddability and names no
licensing statement for plugins. The plugin API specification
(`docs/specs/plugin-api.md`, section "Licensing: PLG-001 options") drafted
three options with their grounds, references, and consequences: a plugin is a
derivative work; the world is an interface boundary and plugins may carry any
licence under a GPL section 7 permission; a conditional linking exception with
an LGPL-style interface layer.

## Decision

The head ruled for the first option. A plugin component distributed for use
with the engine MUST be licensed under GPL-3.0-or-later or under a licence
compatible with it, and its corresponding source MUST be available to every
recipient of the component. Closed-source plugins are excluded. The interface
artifacts (`wit/`, the SDK crate, and the template) stay GPL-3.0-or-later by
default. Clause PLG-001 of the plugin API specification carries the statement.
This record is the project's policy, not legal advice.

## Consequences

### Positive

- First-party plugins change nothing: they carry GPL-3.0-or-later like every
  crate, and the SDK and template can too.
- Free-software plugins, including those under permissive licences such as
  MIT or Apache-2.0, are admitted; each ships source, since a Wasm binary alone
  is not source.
- The simplest statement: no additional permission, exception file, or
  interface-layer licence to maintain.

### Negative

- Closed-source and paid-binary plugins are excluded, as are plugins under
  licences that are not GPL-compatible, for example licences that forbid
  commercial use: the narrowest ecosystem of the three options.
- The policy asserts the stricter reading of an unsettled legal question (the
  FSF itself calls the line between one program and two a question for
  courts), so a later ruling may relax it; relaxing is a new decision.

## Open

Whether the boundary should ever admit WASI-built guests through deterministic
host stubs is recorded in the specification's "Questions for the head" and
remains open; the working policy keeps the boundary as it is, with the SDK
template hiding the compile target.

## Alternatives Considered

- The world as an interface boundary with a section 7 permission, admitting
  plugins under any licence: rejected by the ruling; it would have required
  permissive licences on the interface artifacts and a maintained permission.
- A conditional linking exception with LGPL-style conditions on the interface
  artifacts a plugin embeds: rejected by the ruling; it constrains the SDK's
  design the most and adds obligations modders must understand.

## References

- The options as drafted: `docs/specs/plugin-api.md`, section "Licensing:
  PLG-001 options".
- ADR-0005 (the project licence) and ADR-0001 (the plugin boundary).
- GNU GPL FAQ: https://www.gnu.org/licenses/gpl-faq.html#GPLPlugins,
  https://www.gnu.org/licenses/gpl-faq.html#GPLAndPlugins,
  https://www.gnu.org/licenses/gpl-faq.html#IfInterpreterIsGPL,
  https://www.gnu.org/licenses/gpl-faq.html#MereAggregation.
- GNU GPL version 3, section 7: https://www.gnu.org/licenses/gpl-3.0.html#section7
