#!/usr/bin/env python3
"""Evaluator for the footprint clause of docs/specs/sim.md (SIM-047).

Reads mohodata.scd!lua/footprints.lua in memory and prints identifiers and counts only.
"""

import os
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

SLOPED = (
    "Vehicle1x1 Vehicle2x2 Vehicle5x5 Amphibious1x1 Amphibious3x3 Amphibious6x6 "
    "WaterLand1x1 WaterLand2x2 WaterLand3x3 WaterLand5x5"
).split()
FOOTPRINTS = 20

_NAME = re.compile(r"Name\s*=\s*['\"](\w+)['\"]")
_SLOPE = re.compile(r"MaxSlope\s*=\s*([\d.]+)")
_CAPS = re.compile(r"Caps\s*=\s*([A-Z|]+)")


def max_slope(g: S.Game) -> None:
    sloped, land, names = [], [], []
    for line in g.text("mohodata", "lua/footprints.lua").splitlines():
        name = _NAME.search(line)
        if name is None:
            continue
        names.append(name.group(1))
        slope, caps = _SLOPE.search(line), _CAPS.search(line)
        if slope is not None and float(slope.group(1)) == 0.75:
            sloped.append(name.group(1))
        elif slope is not None:
            raise S.Fail(f"{name.group(1)} has MaxSlope {slope.group(1)}, the clause pins 0.75")
        if caps is not None and "LAND" in caps.group(1).split("|"):
            land.append(name.group(1))
    S.expect("footprints", len(names), FOOTPRINTS)
    S.expect("MaxSlope_0.75_entries", len(sloped), len(SLOPED))
    S.expect("MaxSlope_names", S.words(sloped), S.words(SLOPED))
    S.expect("land_footprints_without_MaxSlope", S.words(set(land) - set(sloped)), "")


if __name__ == "__main__":
    sys.exit(S.run({"SIM-047": max_slope}, sys.argv[1:]))
