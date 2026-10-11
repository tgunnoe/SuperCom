#!/usr/bin/env python3
"""Evaluator for the build-mechanics clauses of docs/specs/sim.md (SIM-010, SIM-011)."""

import os
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S


def expressions(g: S.Game) -> dict[str, list[str]]:
    found = {}
    for uid, bp in g.units().items():
        raw = S.block(bp, "Economy").get("BuildableCategory")
        if isinstance(raw, S.Table) and raw.arr:
            found[uid] = [e for e in raw.arr if isinstance(e, str)]
    return found


def buildable_expressions(g: S.Game) -> None:
    exprs = expressions(g)
    units = g.units()
    vocab = set().union(*[S.categories(bp) for bp in units.values()])
    ids = {u.lower() for u in units}
    tokens = {t for es in exprs.values() for e in es for t in e.split()}
    stray = sorted(t for t in tokens if t not in vocab and t.lower() not in ids)
    S.expect("builders_with_BuildableCategory", len(exprs), 105)
    S.expect("expressions", sum(len(es) for es in exprs.values()), 215)
    S.expect("distinct_identifiers", len(tokens), 68)
    S.expect("identifiers_that_are_blueprint_ids", sum(t not in vocab and t.lower() in ids for t in tokens), 36)
    S.expect("identifiers_that_are_neither", stray, ["CYBRANMOBILEMISSILE", "URL0001UPGRADE"])
    S.expect("expressions_with_operator_characters", sum(bool(re.search(r"[^A-Za-z0-9_\s]", e)) for es in exprs.values() for e in es), 0)


def buildable_sets(g: S.Game) -> None:
    exprs = expressions(g)
    units = g.units()
    cats = {u: S.categories(bp) for u, bp in units.items()}

    def selects(expr: str, uid: str) -> bool:
        return all(t in cats[uid] or t == uid.lower() for t in expr.split())

    empty = sorted(e for es in exprs.values() for e in es if not any(selects(e, u) for u in units))
    built_by = {u: sum(any(selects(e, u) for e in es) for es in exprs.values()) for u in units}
    marked = [u for u in units if any(c.startswith("BUILTBY") for c in cats[u])]
    S.expect("expressions_selecting_no_blueprint", len(empty), 14)
    S.expect("blueprints_buildable_by_some_builder", sum(1 for n in built_by.values() if n), 370)
    S.expect("blueprints_buildable_by_no_builder", sum(1 for n in built_by.values() if not n), 198)
    S.expect("BUILTBY_blueprints_unbuildable", sum(1 for u in marked if not built_by[u]), 0)
    S.expect("COMMAND_blueprints_buildable", sum(1 for u in units if "COMMAND" in cats[u] and built_by[u]), 0)


if __name__ == "__main__":
    sys.exit(S.run({"SIM-010": buildable_expressions, "SIM-011": buildable_sets}, sys.argv[1:]))
