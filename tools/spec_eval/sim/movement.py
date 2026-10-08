#!/usr/bin/env python3
"""Evaluator for the movement and intel clauses of docs/specs/sim.md (SIM-029, SIM-030, SIM-032)."""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

MOTION = {"RULEUMT_Air": 60, "RULEUMT_Amphibious": 17, "RULEUMT_AmphibiousFloating": 8, "RULEUMT_Hover": 19, "RULEUMT_Land": 50, "RULEUMT_None": 374, "RULEUMT_SurfacingSub": 13, "RULEUMT_Water": 27}
LAYERS = ["LAYER_Air", "LAYER_Land", "LAYER_Orbit", "LAYER_Seabed", "LAYER_Sub", "LAYER_Water"]
INTEL = {"VisionRadius": 391, "WaterVisionRadius": 70, "RadarRadius": 57, "SonarRadius": 67, "OmniRadius": 17, "JamRadius": 7, "Cloak": 4, "RadarStealth": 15, "SonarStealth": 9}


def motion(g: S.Game) -> None:
    kinds = Counter(S.block(bp, "Physics").get("MotionType") for bp in g.units().values())
    S.expect("MotionType", dict(sorted(kinds.items())), MOTION)
    S.expect("distinct_MotionType", len(kinds), 8)


def layers(g: S.Game) -> None:
    caps = [S.block(bp, "Physics", "BuildOnLayerCaps") for bp in g.units().values()]
    S.expect("blueprints_with_BuildOnLayerCaps", sum(1 for c in caps if c), 568)
    S.expect("BuildOnLayerCaps_keys", sorted({k for c in caps for k in c}), LAYERS)
    feet = [S.block(bp, "Footprint") for bp in g.units().values()]
    feet = [f for f in feet if f]
    S.expect("blueprints_with_Footprint", len(feet), 402)
    S.expect("distinct_Footprint_sizes", len({(f.get("SizeX"), f.get("SizeZ")) for f in feet}), 24)
    events = ("OnLayerChange", "OnMotionHorzEventChange", "OnMotionVertEventChange", "OnMotionTurnEventChange", "OnTerrainTypeChange")
    S.need_defs(g, "lua", "lua/sim/Unit.lua", events, "unit_motion_names")


def intel(g: S.Game) -> None:
    blocks = [b for b in (S.block(bp, "Intel") for bp in g.units().values()) if b]
    S.expect("blueprints_with_Intel", len(blocks), 395)
    for key, want in INTEL.items():
        S.expect(key, sum(key in b for b in blocks), want)
    names = ("SetupIntel", "EnableUnitIntel", "DisableUnitIntel", "OnIntelEnabled", "OnIntelDisabled", "OnDetectedBy")
    S.need_defs(g, "lua", "lua/sim/Unit.lua", names, "unit_intel_names")


if __name__ == "__main__":
    sys.exit(S.run({"SIM-029": motion, "SIM-030": layers, "SIM-032": intel}, sys.argv[1:]))
