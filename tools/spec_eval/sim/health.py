#!/usr/bin/env python3
"""Evaluator for the health, veterancy, and wreckage clauses of docs/specs/sim.md (SIM-025 to SIM-027)."""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

ACU_HEALTH = {"UEL0001": (12000, 10), "UAL0001": (11000, 10), "URL0001": (10000, 15), "XSL0001": (11500, 10)}
LEVELS = tuple(f"Level{i}" for i in range(1, 6))


def health(g: S.Game) -> None:
    blocks = [S.block(bp, "Defense") for bp in g.units().values()]
    for key, want in (("Health", 568), ("MaxHealth", 568), ("RegenRate", 563)):
        S.expect(f"{key}_blueprints", sum(key in b for b in blocks), want)
    for uid, want in ACU_HEALTH.items():
        d = S.block(g.units()[uid], "Defense")
        S.expect(uid, (d.get("MaxHealth"), d.get("RegenRate")), want)


def veterancy(g: S.Game) -> None:
    vets = [S.block(bp, "Veteran") for bp in g.units().values()]
    vets = [v for v in vets if v]
    tuples = {tuple(v.get(k) for k in LEVELS) for v in vets}
    S.expect("blueprints_with_Veteran", len(vets), 193)
    S.expect("with_all_five_levels", sum(all(k in v for k in LEVELS) for v in vets), 193)
    S.expect("distinct_threshold_tuples", len(tuples), 20)
    S.expect("tuples_strictly_ascending", all(all(a < b for a, b in zip(t, t[1:])) for t in tuples), True)
    regen = [S.block(bp, "Buffs", "Regen") for bp in g.units().values()]
    S.expect("blueprints_with_Buffs_Regen", sum(1 for r in regen if r), 192)
    S.expect("Regen_with_all_five_levels", sum(all(k in r for k in LEVELS) for r in regen if r), 192)
    S.need_defs(g, "lua", "lua/sim/Unit.lua", ("AddKills", "SetVeterancy", "GetVeteranLevel", "CheckVeteranLevel", "SetVeteranLevel", "CreateVeterancyBuff", "PlayVeteranFx"), "unit_veterancy_names")


def wreckage(g: S.Game) -> None:
    wr = [S.block(bp, "Wreckage") for bp in g.units().values()]
    wr = [w for w in wr if w]
    keys = ("Blueprint", "EnergyMult", "HealthMult", "MassMult", "ReclaimTimeMultiplier", "WreckageLayers")
    S.expect("blueprints_with_Wreckage", len(wr), 504)
    S.expect("with_all_six_keys", sum(all(k in w for k in keys) for w in wr), 504)
    S.expect("layer_keys", sorted({k for w in wr for k in w["WreckageLayers"]}), ["Air", "Land", "Seabed", "Sub", "Water"])
    paths = Counter(w["Blueprint"] for w in wr)
    S.expect("wreckage_blueprint_paths", len(paths), 2)
    S.expect("unresolved_wreckage_paths", sorted(p for p in paths if p.lstrip("/").lower() not in g.index()), [])
    death = ("OnKilled", "DoDeathWeapon", "CreateWreckage", "CreateWreckageProp", "DeathThread", "OnDestroy", "CreateDestructionEffects")
    S.need_defs(g, "lua", "lua/sim/Unit.lua", death, "unit_death_names")
    S.need_defs(g, "mohodata", "lua/system/Blueprints.lua", ("ExtractWreckageBlueprint",), "blueprint_wreckage_names")


if __name__ == "__main__":
    sys.exit(S.run({"SIM-025": health, "SIM-026": veterancy, "SIM-027": wreckage}, sys.argv[1:]))
