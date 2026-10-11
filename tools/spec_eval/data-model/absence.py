#!/usr/bin/env python3
"""Evaluator for the absence-census clause of docs/specs/data-model.md (DAT-006).

Counts, over the installed blueprints, where the typed model's optional scalars and
empty lists come from: the fields the installed data omits. Reads archive members in
memory through the simlib data-subset reader and prints counts only.
"""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "formats"))
import fmtlib as F

S = F.S


def census(game) -> None:
    units = list(game.units().values())
    F.expect("units", len(units), 568)
    F.expect("units_without_BuildTime", sum("BuildTime" not in S.block(u, "Economy") for u in units), 16)
    intel = [S.block(u, "Intel") for u in units if "Intel" in u]
    F.expect("intel_blocks", len(intel), 395)
    F.expect("intel_without_VisionRadius", sum("VisionRadius" not in i for i in intel), 4)
    weapons = [w for u in units for w in S.block(u, "Weapon").arr]
    F.expect("weapon_entries", len(weapons), 494)
    no_rate = [w for w in weapons if "RateOfFire" not in w]
    no_radius = [w for w in weapons if "MaxRadius" not in w]
    F.expect("weapons_without_RateOfFire", len(no_rate), 91)
    F.expect("those_with_WeaponCategory_Death", sum(w.get("WeaponCategory") == "Death" for w in no_rate), 90)
    F.expect("weapons_without_MaxRadius", len(no_radius), 86)
    F.expect("those_with_WeaponCategory_Death", sum(w.get("WeaponCategory") == "Death" for w in no_radius), 86)
    projectiles = props = bare_projectiles = bare_props = 0
    for archive in ("projectiles", "effects"):
        for name in game.names(archive):
            if name.lower().endswith("_proj.bp"):
                projectiles += 1
                bare_projectiles += "Categories" not in S.parse_table(game.text(archive, name), name)
    for archive in ("env", "meshes", "props"):
        for name in game.names(archive):
            if name.lower().endswith("_prop.bp"):
                props += 1
                bare_props += "Categories" not in S.parse_table(game.text(archive, name), name)
    F.expect("projectiles", projectiles, 365)
    F.expect("projectiles_without_Categories", bare_projectiles, 90)
    F.expect("props", props, 336)
    F.expect("props_without_Categories", bare_props, 37)


CHECKS = {"DAT-006": census}

if __name__ == "__main__":
    sys.exit(S.run(CHECKS, sys.argv[1:]))
