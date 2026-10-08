# ADR-0004: Agent tier routing

**Status:** ACCEPTED

**Date:** 2026-10-07

---

## Context

The project is built with AI agents governed by predicate. Predicate routes
work by tier: architect-class walkers map state spaces, emit boundaries, and
judge landed work; worker-class walkers execute saturated boundaries under one
discipline; mechanical work goes to the cheapest tier (predicate `rules.md`
§6). The head runs on a Claude Max plan and wants both speed and credit
efficiency, with reviews of major features as pull requests.

## Decision

| Role                                                                                                              | Model            | Why                                                                     |
| :---------------------------------------------------------------------------------------------------------------- | :--------------- | :---------------------------------------------------------------------- |
| Composer (the interactive session): bounding, campaign survey and routing, reconciliation, council moderation     | Claude Fable 5.1 | Judgment is where the expensive tokens pay off                          |
| Council seats: `predicate-architect-seat`, `-lead-maintainer-seat`, `-process-auditor-seat`, `-hacker-seat`       | Opus             | Predicate's default; architect-class review at lower cost               |
| Workers and reviewers: `predicate-core-worker`, `-spec-worker`, `-test-worker`, `-refuter-reviewer`, and the rest | Sonnet           | Predicate's default; verified by compiler, clippy, tests, and contracts |
| Survey: `predicate-survey-worker`                                                                                 | Haiku            | Predicate's default; read-only mapping with file:line evidence          |

- Background dispatches name the role (`subagent_type: "predicate-<role>"`)
  and inherit that role's model. A campaign boundary may elevate one dispatch
  with a stated reason.
- Human seams stay human: goal confirmation, campaign and routing approval,
  load-bearing DAG amendments, merge consent, release and push.

## Consequences

### Positive

- Fable is spent on judgment, not on mechanical work.
- Machine evaluators, not expensive reviewers, verify worker output.

### Negative

- Opus seats may miss subtleties Fable would catch; such misses surface as
  rework at reconcile rather than being prevented.

---

## Alternatives Considered

| Alternative                         | Reason Rejected                                                      |
| :---------------------------------- | :------------------------------------------------------------------- |
| Fable for all architect-class roles | Better judging at a noticeably higher cost per reconcile.            |
| Cheapest viable (Haiku workers)     | Highest rework risk on engine code; the savings leak back as rework. |

---

## References

- AGENTS.md: Operational entrypoint.
- Predicate `rules.md` §2 (Invariant 6) and §6; `conditioning/install.sh`
  (`agent_model`): https://github.com/nrdxp/predicate
