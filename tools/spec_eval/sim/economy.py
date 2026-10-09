#!/usr/bin/env python3
"""Evaluator for the economy clauses of docs/specs/sim.md (SIM-006 to SIM-008)."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

COMMANDERS = ("UEL0001", "UAL0001", "URL0001", "XSL0001")
ACU_ECONOMY = {"BuildRate": 10, "BuildCostMass": 18000, "BuildCostEnergy": 5000000, "BuildTime": 60000, "ProductionPerSecondMass": 1, "ProductionPerSecondEnergy": 20, "StorageMass": 650, "StorageEnergy": 4000}


def cost_fields(g: S.Game) -> None:
    units = g.units()
    blocks = [S.block(bp, "Economy") for bp in units.values()]
    S.expect("unit_blueprints", len(units), 568)
    S.expect("with_Economy", sum(1 for b in blocks if b), 568)
    for key, want in (("BuildCostMass", 568), ("BuildCostEnergy", 568), ("BuildTime", 552), ("BuildRate", 128)):
        S.expect(f"{key}_blueprints", sum(1 for b in blocks if key in b), want)


def resource_keys(g: S.Game) -> None:
    census = S.key_census(S.block(bp, "Economy") for bp in g.units().values())
    S.expect("distinct_Economy_keys", len(census), 32)
    named = sorted(k for k in census if "Mass" in k or "Energy" in k)
    S.expect("keys_naming_Mass_or_Energy", len(named), 19)
    for key, want in (("ProductionPerSecondMass", 28), ("ProductionPerSecondEnergy", 24), ("StorageMass", 62), ("StorageEnergy", 62), ("MaintenanceConsumptionPerSecondEnergy", 84)):
        S.expect(key, census[key], want)
    S.expect("MaintenanceConsumptionPerSecondMass", census["MaintenanceConsumptionPerSecondMass"], 0)


def commander_economy(g: S.Game) -> None:
    for uid in COMMANDERS:
        block = S.block(g.units().get(uid, S.Table()), "Economy")
        S.expect(uid, {k: block.get(k) for k in ACU_ECONOMY}, ACU_ECONOMY)


if __name__ == "__main__":
    sys.exit(S.run({"SIM-006": cost_fields, "SIM-007": resource_keys, "SIM-008": commander_economy}, sys.argv[1:]))
