#!/usr/bin/env python3
"""Evaluator for the vision and victory game-data clause of docs/specs/sim.md (SIM-082).

Reads the victory script, the brain script, the unit Lua, and the unit blueprints in
memory and prints identifiers, line numbers, and counts only; no line of the installed
game is echoed.
"""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

# (line, identifier or token the line names) of lua/victory.lua of lua.scd.
VICTORY_LINES = (
    (2, "CheckVictory"),
    (5, "demoralization"),
    (7, "COMMAND"),
    (8, "domination"),
    (10, "STRUCTURE"),
    (10, "ENGINEER"),
    (10, "WALL"),
    (11, "eradication"),
    (13, "ALLUNITS"),
    (16, "return"),
    (23, "while"),
    (28, "ArmyIsCivilian"),
    (29, "GetCurrentUnits"),
    (30, "OnDefeat"),
    (39, "empty"),
    (40, "CallEndGame"),
    (89, "CallEndGame"),
)
BRAIN_LINES = ((782, "OnDefeat"), (783, "SetArmyOutOfGame"))
# lua/sim/Unit.lua of lua.scd: a shadowed member (AGENTS.md U9), lua.scd's copy.
VISION_LINES = ((183, "VisionRadius"), (185, "SetIntelRadius"))
# Blueprints with an Intel block and no VisionRadius.
WITHOUT_VISION = ["XRB3301", "XSC9002", "XSC9010", "XSC9011"]


def line_names(g: S.Game, archive: str, member: str, lines) -> None:
    winner = g.index().get(member.lower())
    if winner != archive:
        raise S.Fail(f"{member} is won by {winner}, not {archive}")
    text = g.text(archive, member).splitlines()
    for number, name in lines:
        if name not in text[number - 1]:
            raise S.Fail(f"line {number} of {member} does not name {name}")
    S.show(f"{member}_lines_checked", [n for n, _ in lines])


def victory_and_vision(g: S.Game) -> None:
    line_names(g, "lua", "lua/victory.lua", VICTORY_LINES)
    line_names(g, "lua", "lua/aibrain.lua", BRAIN_LINES)
    line_names(g, "lua", "lua/sim/Unit.lua", VISION_LINES)
    shadowed = sorted(a for a in ("lua", "mohodata") if "lua/sim/unit.lua" in {n.lower() for n in g.names(a)})
    S.expect("Unit_lua_archives", shadowed, ["lua", "mohodata"])
    intel = {u: S.block(bp, "Intel") for u, bp in g.units().items()}
    intel = {u: b for u, b in intel.items() if b}
    S.expect("blueprints_with_Intel", len(intel), 395)
    S.expect("with_VisionRadius", sum("VisionRadius" in b for b in intel.values()), 391)
    S.expect("without_VisionRadius", sorted(u for u, b in intel.items() if "VisionRadius" not in b), WITHOUT_VISION)


if __name__ == "__main__":
    sys.exit(S.run({"SIM-082": victory_and_vision}, sys.argv[1:]))
