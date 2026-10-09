#!/usr/bin/env python3
"""Evaluator for the tick-model clauses of docs/specs/sim.md (SIM-003)."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S


def tick_functions(g: S.Game) -> None:
    names = ("GetSimTicksPerSecond", "SecondsPerTick", "GetGameTimeSeconds", "GetGameTick")
    hits = S.member_hits(g, "lua", names)
    for name in names:
        S.expect(f"{name}_members", len(hits[name]), {"GetSimTicksPerSecond": 1, "SecondsPerTick": 1, "GetGameTimeSeconds": 10, "GetGameTick": 0}[name])
    union = {m for n in names[:3] for m in hits[n]}
    S.expect("members_citing_tick_functions", len(union), 12)
    S.expect("GetSimTicksPerSecond_member", hits[names[0]], ["lua/ui/game/economy.lua"])
    S.expect("SecondsPerTick_member", hits[names[1]], ["lua/sim/tasks/EnhanceTask.lua"])


if __name__ == "__main__":
    sys.exit(S.run({"SIM-003": tick_functions}, sys.argv[1:]))
