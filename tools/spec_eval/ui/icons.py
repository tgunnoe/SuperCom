#!/usr/bin/env python3
"""Evaluator for the strategic icon clauses of docs/specs/ui.md (HUD-012 to HUD-014)."""

import os
import re
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import uilib as U

PREFIX = "textures/ui/common/game/strategicicons/"
STATES = ["over", "rest", "selected", "selectedover"]
STEM = re.compile(r"^(icon_(?P<kind>[a-z0-9]+)_(?P<role>[a-z]+))_(?P<state>over|rest|selected|selectedover)\.dds$")


def textures(g: U.Game) -> list[str]:
    """Lower-case file names of the .dds members under the icon folder of textures.scd."""
    holders = dict(g.holders(PREFIX))
    if set(holders) != {"textures"}:
        raise U.Fail(f"the icon folder is held by {sorted(holders)}, not by textures alone")
    return sorted(n[len(PREFIX) :].lower() for n in g.members("textures", PREFIX) if n.lower().endswith(".dds"))


def stems(names: list[str]) -> tuple[dict[str, set[str]], list[str]]:
    """Icon stem -> the states present, and the names that fit no stem."""
    found: dict[str, set[str]] = {}
    odd = []
    for name in names:
        match = STEM.match(name)
        if match:
            found.setdefault(match.group(1), set()).add(match["state"])
        else:
            odd.append(name)
    return found, odd


def files(g: U.Game) -> None:
    names = textures(g)
    found, odd = stems(names)
    U.expect("dds_files", len(names), 1102)
    U.expect("icon_stems", len(found), 270)
    U.expect("stems_with_over_rest_selected", sum({"over", "rest", "selected"} <= s for s in found.values()), 270)
    U.expect("stems_with_all_four_states", sum(set(STATES) <= s for s in found.values()), 270)
    U.expect("state_files", sum(len(s) for s in found.values()), 1080)
    U.expect("other_files", len(odd), 22)
    kinds = {STEM.match(n)["kind"] for n in names if STEM.match(n)}
    roles = {STEM.match(n)["role"] for n in names if STEM.match(n)}
    U.expect("kinds", len(kinds), 40)
    U.expect("roles", len(roles), 24)
    U.expect("other_files_without_a_role", len([n for n in odd if n.startswith("icon_")]), 8)
    U.expect("other_files_ping", len([n for n in odd if n.startswith("strat_")]), 12)
    U.expect("other_files_pause_and_stun", sorted(n for n in odd if not n.startswith(("icon_", "strat_"))), ["pause_rest.dds", "stunned_rest.dds"])


def link(g: U.Game) -> None:
    found, _ = stems(textures(g))
    units = g.units()
    named = {u: bp["icon"] for u, bp in units.items() if bp["icon"]}
    values = Counter(named.values())
    U.expect("blueprints", len(units), 568)
    U.expect("blueprints_with_StrategicIconName", len(named), 550)
    U.expect("blueprints_without", len(units) - len(named), 18)
    U.expect("distinct_StrategicIconName", len(values), 100)
    U.expect("names_not_of_the_form_icon_kind_role", sorted(v for v in values if not re.fullmatch(r"icon_[a-z0-9]+_[a-z]+", v)), [])
    U.expect("names_without_an_icon_stem", sorted(v for v in values if v not in found), [])
    U.expect("icon_stems_no_blueprint_names", len(set(found) - set(values)), 170)


def complete(g: U.Game) -> None:
    found, _ = stems(textures(g))
    named = {u: bp["icon"] for u, bp in g.units().items() if bp["icon"]}
    short = sorted(u for u, icon in named.items() if set(STATES) - found.get(icon, set()))
    U.expect("named_icons", len(named), 550)
    U.expect("named_icons_missing_a_state", short, [])


if __name__ == "__main__":
    sys.exit(U.run({"HUD-012": files, "HUD-013": link, "HUD-014": complete}, sys.argv[1:]))
