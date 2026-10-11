#!/usr/bin/env python3
"""Evaluator for the victory and initial-state clauses of docs/specs/sim.md (SIM-034, SIM-035)."""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

FACTIONS = {"UEL0001": "UEF", "UAL0001": "Aeon", "URL0001": "Cybran", "XSL0001": "Seraphim"}


def victory(g: S.Game) -> None:
    text = g.text("lua", "lua/victory.lua")
    for word in ("demoralization", "domination", "eradication"):
        S.expect(word, text.count(word) > 0, True)
    S.expect("sandbox", text.count("sandbox"), 0)
    S.expect("Options_Victory_references", text.count("Options.Victory"), 3)
    S.need_defs(g, "lua", "lua/victory.lua", ("CheckVictory", "CallEndGame"), "victory_names")


def initial_state(g: S.Game) -> None:
    units = g.units()
    commanders = {u: S.block(bp, "General").get("FactionName") for u, bp in units.items() if "COMMAND" in S.categories(bp)}
    S.expect("COMMAND_blueprints", commanders, FACTIONS)
    brain = ("OnCreateHuman", "OnCreateAI", "OnSpawnPreBuiltUnits", "InitializeEconomyState", "OnDefeat", "OnVictory", "OnDraw")
    S.need_defs(g, "lua", "lua/aibrain.lua", brain, "brain_names")
    S.need_defs(g, "mohodata", "lua/sim/ScenarioUtilities.lua", ("InitializeArmies", "CreateInitialArmyGroup", "CreateResources", "CommanderWarpDelay"), "scenario_names")


if __name__ == "__main__":
    sys.exit(S.run({"SIM-034": victory, "SIM-035": initial_state}, sys.argv[1:]))
