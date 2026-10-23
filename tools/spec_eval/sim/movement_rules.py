#!/usr/bin/env python3
"""Evaluator for the movement game-data clauses of docs/specs/sim.md (SIM-062, SIM-063).

Reads mohodata.scd!lua/footprints.lua and the unit blueprints in memory and prints
identifiers and counts only; no line of the installed game is echoed.
"""

import os
import re
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

# name -> (caps, MaxSlope, MinWaterDepth, MaxWaterDepth); None where the row names none.
FOOTPRINTS = {
    "Vehicle1x1": ("LAND", 0.75, None, 0.05),
    "Vehicle2x2": ("LAND", 0.75, None, 0.05),
    "Vehicle5x5": ("LAND", 0.75, None, 0.05),
    "Amphibious1x1": ("LAND|SEABED", 0.75, None, 25.0),
    "Amphibious3x3": ("LAND|SEABED", 0.75, None, 25.0),
    "Amphibious6x6": ("LAND|SEABED", 0.75, None, 25.0),
    "WaterLand1x1": ("LAND|WATER", 0.75, 0.1, 1.0),
    "WaterLand2x2": ("LAND|WATER", 0.75, 0.1, 1.0),
    "WaterLand3x3": ("LAND|WATER", 0.75, 0.0, 5.0),
    "WaterLand5x5": ("LAND|WATER", 0.75, 0.0, 5.0),
    "SurfacingSub2x2": ("SUB|WATER", None, 1.5, None),
    "SurfacingSub3x3": ("SUB|WATER", None, 1.5, None),
    "SurfacingSub4x4": ("SUB|WATER", None, 1.5, None),
    "SurfacingSub12x12": ("SUB|WATER", None, 1.5, None),
    "Water1x1": ("WATER", None, 1.5, None),
    "Water3x3": ("WATER", None, 0.25, None),
    "Water4x4": ("WATER", None, 1.5, None),
    "Water6x6": ("WATER", None, 1.5, None),
    "Water8x8": ("WATER", None, 1.5, None),
    "Water11x11": ("WATER", None, 1.5, None),
}
FIRST_LINE, LAST_LINE = 21, 44
SCALARS = {"slope": "MaxSlope", "min": "MinWaterDepth", "max": "MaxWaterDepth"}
FIELDS = "MaxSpeed TurnRate MaxAcceleration MaxBrake Elevation MinSpeedPercent".split()
MOTION = {
    "RULEUMT_Air": 60,
    "RULEUMT_Land": 50,
    "RULEUMT_Water": 27,
    "RULEUMT_Hover": 19,
    "RULEUMT_Amphibious": 17,
    "RULEUMT_SurfacingSub": 13,
    "RULEUMT_AmphibiousFloating": 8,
}
# Per field: motion type -> number of movers that carry it.
CARRIED = {
    "MaxSpeed": MOTION,
    "TurnRate": {**MOTION, "RULEUMT_Air": 13},
    "MaxAcceleration": {**MOTION, "RULEUMT_Air": 13},
    "MaxBrake": {**MOTION, "RULEUMT_Air": 0, "RULEUMT_Land": 45, "RULEUMT_Amphibious": 11},
}
ELEVATION = {"RULEUMT_Air": 60, "RULEUMT_Hover": 19, "RULEUMT_SurfacingSub": 13, "RULEUMT_Land": 2, "RULEUMT_Amphibious": 1}
RANGES = {
    ("MaxSpeed", "RULEUMT_Land"): (0.75, 5),
    ("MaxSpeed", "RULEUMT_Water"): (2.5, 8),
    ("MaxSpeed", "RULEUMT_Hover"): (1.9, 5),
    ("Elevation", "RULEUMT_Air"): (0.4, 75),
    ("Elevation", "RULEUMT_Hover"): (0.11, 2.5),
    ("Elevation", "RULEUMT_SurfacingSub"): (-5, -1.5),
    ("TurnRate", "RULEUMT_Land"): (27, 180),
}


def footprint_rows(g: S.Game) -> dict[str, tuple]:
    lines = g.text("mohodata", "lua/footprints.lua").splitlines()
    rows: dict[str, tuple] = {}
    for number, line in enumerate(lines, 1):
        name = re.search(r"Name\s*=\s*['\"](\w+)['\"]", line)
        if name is None:
            continue
        if not FIRST_LINE <= number <= LAST_LINE:
            raise S.Fail(f"footprint {name.group(1)} on line {number}, outside {FIRST_LINE} to {LAST_LINE}")
        caps = re.search(r"Caps\s*=\s*([A-Z|]+)", line)
        found = {}
        for key, field in SCALARS.items():
            hit = re.search(rf"{field}\s*=\s*([\d.]+)", line)
            found[key] = float(hit.group(1)) if hit else None
        rows[name.group(1)] = (caps.group(1) if caps else None, found["slope"], found["min"], found["max"])
    return rows


def footprint_table(g: S.Game) -> None:
    rows = footprint_rows(g)
    S.expect("footprints", len(rows), 20)
    S.expect("footprint_names", S.words(sorted(rows)), S.words(sorted(FOOTPRINTS)))
    for name, want in FOOTPRINTS.items():
        S.expect(f"{name}_caps_slope_min_max", rows[name], want)
    S.expect("members_holding_footprints_lua", sorted(a for a in ("lua", "mohodata") if "lua/footprints.lua" in {n.lower() for n in g.names(a)}), ["mohodata"])


def physics(bp) -> S.Table:
    return S.block(bp, "Physics")


def census(g: S.Game) -> None:
    units = g.units()
    movers = {u: physics(bp) for u, bp in units.items() if physics(bp).get("MotionType") not in (None, "RULEUMT_None")}
    S.expect("movers", len(movers), 194)
    S.expect("mover_motion_types", dict(Counter(p["MotionType"] for p in movers.values())), MOTION)
    for field, want in CARRIED.items():
        have = Counter(p["MotionType"] for p in movers.values() if field in p)
        S.expect(f"{field}_carried_by", {k: have.get(k, 0) for k in MOTION}, {k: want.get(k, 0) for k in MOTION})
    S.expect("movers_without_MaxBrake", sum("MaxBrake" not in p for p in movers.values()), 71)
    have = Counter(p["MotionType"] for p in movers.values() if "Elevation" in p)
    S.expect("Elevation_carried_by", dict(sorted(have.items())), dict(sorted(ELEVATION.items())))
    for (field, motion), want in RANGES.items():
        values = [p[field] for p in movers.values() if field in p and p["MotionType"] == motion]
        S.expect(f"{field}_range_{motion}", (min(values), max(values)), want)
    other = [p["TurnRate"] for p in movers.values() if "TurnRate" in p and p["MotionType"] != "RULEUMT_Land"]
    S.expect("TurnRate_range_not_land", (min(other), max(other)), (12, 180))
    minimum = Counter((p["MotionType"] == "RULEUMT_Air", p["MinSpeedPercent"]) for p in movers.values() if "MinSpeedPercent" in p)
    S.expect("MinSpeedPercent_nonzero_air_units", minimum[(True, 0.85)], 16)
    S.expect("MinSpeedPercent_values", sorted({k[1] for k in minimum}), [0, 0.85])
    S.expect("MinSpeedPercent_nonzero_off_air", sum(n for k, n in minimum.items() if not k[0] and k[1] != 0), 0)
    S.expect("Footprint_blocks", sum(bool(S.block(bp, "Footprint")) for bp in units.values()), 402)
    sizes = [S.block(bp, "Footprint").get("SizeX") for bp in units.values() if S.block(bp, "Footprint").get("SizeX") is not None]
    S.expect("Footprint_SizeX_range", (min(sizes), max(sizes)), (0, 20))


if __name__ == "__main__":
    sys.exit(S.run({"SIM-062": footprint_table, "SIM-063": census}, sys.argv[1:]))
