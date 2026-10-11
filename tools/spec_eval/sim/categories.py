#!/usr/bin/env python3
"""Evaluator for the category and tech-tier clauses of docs/specs/sim.md (SIM-013 to SIM-015)."""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S


def vocabulary(g: S.Game) -> None:
    cats = {k: S.categories(v) for k, v in g.units().items()}
    every = set().union(*cats.values())
    mixed = sorted(c for c in every if c != c.upper())
    S.expect("unit_blueprints", len(cats), 568)
    S.expect("distinct_categories", len(every), 152)
    S.expect("upper_case_categories", len(every) - len(mixed), 151)
    S.expect("mixed_case_categories", mixed, ["Cybran"])
    S.expect("units_with_Cybran", sum("Cybran" in c for c in cats.values()), 9)


def tiers(g: S.Game) -> None:
    names = ("TECH1", "TECH2", "TECH3", "EXPERIMENTAL")
    shape = Counter(tuple(sorted(S.categories(bp) & set(names))) for bp in g.units().values())
    for name, want in zip(names, (127, 134, 119, 17)):
        S.expect(name, shape[(name,)], want)
    S.expect("untiered", shape[()], 171)
    S.expect("units_with_two_tiers", sum(v for k, v in shape.items() if len(k) > 1), 0)


def kinds_and_layers(g: S.Game) -> None:
    cats = [S.categories(bp) for bp in g.units().values()]
    for name, want in (("MOBILE", 191), ("STRUCTURE", 361), ("LAND", 252), ("AIR", 72), ("NAVAL", 50), ("COMMAND", 4)):
        S.expect(name, sum(name in c for c in cats), want)
    S.expect("MOBILE_and_STRUCTURE", sum("MOBILE" in c and "STRUCTURE" in c for c in cats), 0)
    S.expect("neither_MOBILE_nor_STRUCTURE", sum("MOBILE" not in c and "STRUCTURE" not in c for c in cats), 16)
    S.expect("units_with_two_layers", sum(len(c & {"LAND", "AIR", "NAVAL"}) > 1 for c in cats), 0)


if __name__ == "__main__":
    sys.exit(S.run({"SIM-013": vocabulary, "SIM-014": tiers, "SIM-015": kinds_and_layers}, sys.argv[1:]))
