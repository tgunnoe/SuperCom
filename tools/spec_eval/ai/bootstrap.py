#!/usr/bin/env python3
"""Evaluator for the bootstrap clauses of docs/specs/ai.md (SKAI-012, SKAI-013)."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ailib as A

COMMANDERS = {"UEF": "UEL0001", "AEON": "UAL0001", "CYBRAN": "URL0001", "SERAPHIM": "XSL0001"}
# The role queries of SKAI-011, as category sets; no query names a blueprint ID or a faction.
QUERIES = {
    "mass_extractor": {"MASSEXTRACTION", "TECH1"},
    "energy_producer": {"ENERGYPRODUCTION", "TECH1"},
    "land_factory": {"FACTORY", "LAND", "TECH1"},
    "engineer": {"ENGINEER", "TECH1"},
    "combat_unit": {"LAND", "MOBILE", "TECH1", "DIRECTFIRE"},
}
MENU = {"UEF": 45, "AEON": 45, "CYBRAN": 45, "SERAPHIM": 41}
# The blueprints the first four role queries select, by faction; the last role is pinned by count.
FOUND = {
    "UEF": {"mass_extractor": ["UEB1103"], "energy_producer": ["UEB1101", "UEB1102"], "land_factory": ["UEB0101"], "engineer": ["UEL0105"]},
    "AEON": {"mass_extractor": ["UAB1103"], "energy_producer": ["UAB1101", "UAB1102"], "land_factory": ["UAB0101"], "engineer": ["UAL0105"]},
    "CYBRAN": {"mass_extractor": ["URB1103"], "energy_producer": ["URB1101", "URB1102"], "land_factory": ["URB0101"], "engineer": ["URL0105"]},
    "SERAPHIM": {"mass_extractor": ["XSB1103"], "energy_producer": ["XSB1101", "XSB1102"], "land_factory": ["XSB0101"], "engineer": ["XSL0105"]},
}
COMBAT = {"UEF": 3, "AEON": 3, "CYBRAN": 2, "SERAPHIM": 2}


def chain(units: dict, faction: str) -> dict[str, list[str]]:
    """The role -> blueprints map of one faction: the commander's structures for the first
    three roles, the land factory's units for the last two."""
    menu = A.buildable_by(units, COMMANDERS[faction])
    found: dict[str, list[str]] = {}
    for role in ("mass_extractor", "energy_producer", "land_factory"):
        found[role] = sorted(u for u in menu if QUERIES[role] <= units[u]["cats"])
    factory = found["land_factory"]
    if not factory:
        raise A.Fail(f"{faction} commander selects no land factory")
    products = A.buildable_by(units, factory[0])
    for role in ("engineer", "combat_unit"):
        found[role] = sorted(u for u in products if QUERIES[role] <= units[u]["cats"])
    return found


def feasibility(g: A.Game) -> None:
    units = g.units()
    A.guard_sim_reading(units)
    for faction in COMMANDERS:
        menu = A.buildable_by(units, COMMANDERS[faction])
        A.expect(f"{faction}_commander_builds", len(menu), MENU[faction])
        A.expect(f"{faction}_commander_builds_TECH1", sum("TECH1" in units[u]["cats"] for u in menu), 14)
        found = chain(units, faction)
        A.expect(f"{faction}_roles", {r: v for r, v in found.items() if r != "combat_unit"}, FOUND[faction])
        A.expect(f"{faction}_combat_unit", len(found["combat_unit"]), COMBAT[faction])


def faction_agnostic(g: A.Game) -> None:
    units = g.units()
    A.guard_sim_reading(units)
    A.expect("queries", len(QUERIES), 5)
    ids = {u.lower() for u in units}
    A.expect("queries_naming_a_blueprint_or_faction", sum(bool(q & (ids | set(COMMANDERS))) for q in QUERIES.values()), 0)
    for faction in COMMANDERS:
        if faction not in units[COMMANDERS[faction]]["cats"]:
            raise A.Fail(f"{COMMANDERS[faction]} does not carry the category {faction}")
        found = chain(units, faction)
        empty = sorted(role for role, hits in found.items() if not hits)
        foreign = sorted(u for hits in found.values() for u in hits if faction not in units[u]["cats"])
        A.expect(f"{faction}_roles_without_a_candidate", empty, [])
        A.expect(f"{faction}_candidates_of_another_faction", foreign, [])


if __name__ == "__main__":
    sys.exit(A.run({"SKAI-012": feasibility, "SKAI-013": faction_agnostic}, sys.argv[1:]))
