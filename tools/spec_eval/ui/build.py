#!/usr/bin/env python3
"""Evaluator for the build menu measurement of docs/specs/ui.md (HUD-010)."""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import uilib as U

TIERS = ["TECH1", "TECH2", "TECH3", "EXPERIMENTAL"]
COMMANDERS = {
    "UEL0001": {"EXPERIMENTAL": 4, "TECH1": 14, "TECH2": 15, "TECH3": 12},
    "UAL0001": {"EXPERIMENTAL": 4, "TECH1": 14, "TECH2": 14, "TECH3": 13},
    "URL0001": {"EXPERIMENTAL": 4, "TECH1": 14, "TECH2": 15, "TECH3": 12},
    "XSL0001": {"EXPERIMENTAL": 3, "TECH1": 14, "TECH2": 14, "TECH3": 10},
}


def tier(units: dict, ident: str) -> str:
    held = [t for t in TIERS if t in units[ident]["cats"]]
    return "+".join(held) if held else "none"


def menu(g: U.Game) -> None:
    units = g.units()
    U.guard_sim_reading(units)
    builders = [u for u, bp in units.items() if bp["builds"]]
    menus = {b: U.buildable_by(units, b) for b in builders}
    chosen = set().union(*menus.values())
    U.expect("selected_by_a_builder_by_tier", dict(sorted(Counter(tier(units, u) for u in chosen).items())), {"EXPERIMENTAL": 15, "TECH1": 105, "TECH2": 134, "TECH3": 116})
    U.expect("builders_whose_menu_holds_an_ungrouped_entry", sum(any(tier(units, u) not in TIERS for u in m) for m in menus.values()), 0)
    U.expect("largest_menu", max(len(m) for m in menus.values()), 45)
    U.expect("empty_menus", sum(not m for m in menus.values()), 1)
    for commander, by_tier in COMMANDERS.items():
        U.expect(f"{commander}_menu", len(menus[commander]), sum(by_tier.values()))
        U.expect(f"{commander}_menu_by_tier", dict(sorted(Counter(tier(units, u) for u in menus[commander]).items())), by_tier)
    text = g.text("lua", "lua/ui/game/construction.lua")
    absent = sorted(w for w in TIERS + ["StrategicIconName", "IssueCommand"] if U.word_count(text, w) == 0)
    U.expect("construction_lua_names_missing", absent, [])


if __name__ == "__main__":
    sys.exit(U.run({"HUD-010": menu}, sys.argv[1:]))
