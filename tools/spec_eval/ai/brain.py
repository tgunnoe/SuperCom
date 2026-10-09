#!/usr/bin/env python3
"""Evaluator for the Lua-side AI class clause of docs/specs/ai.md (SKAI-009)."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ailib as A

CLASSES = {
    "lua/aibrain.lua": ["AIBrain"],
    "lua/platoon.lua": ["Platoon"],
    "lua/sim/Builder.lua": ["Builder", "PlatoonBuilder", "FactoryBuilder", "EngineerBuilder"],
    "lua/sim/BuilderManager.lua": ["BuilderManager"],
    "lua/sim/EngineerManager.lua": ["EngineerManager"],
    "lua/sim/FactoryBuilderManager.lua": ["FactoryBuilderManager"],
    "lua/sim/PlatoonFormManager.lua": ["PlatoonFormManager"],
    "lua/sim/StrategyManager.lua": ["StrategyManager"],
    "lua/sim/BrainConditionsMonitor.lua": ["BrainConditionsMonitor"],
}


def classes(g: A.Game) -> None:
    A.show("members", len(CLASSES))
    for member, names in CLASSES.items():
        winner = g.index().get(member.lower())
        if winner != "lua":
            raise A.Fail(f"{member} is won by {winner}, not lua")
        missing = sorted(set(names) - A.lua_defs(g.text("lua", member)))
        if missing:
            raise A.Fail(f"gamedata/lua.scd!{member} does not define {' '.join(missing)}")
    A.expect("classes", sum(len(v) for v in CLASSES.values()), 12)


if __name__ == "__main__":
    sys.exit(A.run({"SKAI-009": classes}, sys.argv[1:]))
