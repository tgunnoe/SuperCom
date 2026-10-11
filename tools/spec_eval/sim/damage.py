#!/usr/bin/env python3
"""Evaluator for the damage, armour, and shield clauses of docs/specs/sim.md (SIM-022 to SIM-024)."""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

ARMOUR = {
    "Default": {"Normal": 1.0},
    "Normal": {"Normal": 1.0},
    "Light": {"Normal": 1.0},
    "Commander": {"Normal": 1.0, "Overcharge": 0.033333, "Deathnuke": 0.05},
    "Structure": {"Normal": 1.0, "Overcharge": 0.066666, "Deathnuke": 0.01},
    "Experimental": {"ExperimentalFootfall": 0.0},
}
SHIELD_KEYS = ("ShieldSize", "ShieldMaxHealth", "ShieldRechargeTime", "ShieldEnergyDrainRechargeTime", "ShieldRegenRate", "ShieldRegenStartTime", "ShieldVerticalOffset", "RegenAssistMult")


def armour_table(g: S.Game) -> dict:
    text = g.text("lua", "lua/armordefinition.lua")
    entries = S.parse_assignments(text, "lua/armordefinition.lua")["armordefinition"].arr
    return {e.arr[0]: {k: float(v) for k, v in (s.split() for s in e.arr[1:])} for e in entries}


def armour(g: S.Game) -> None:
    table = armour_table(g)
    used = Counter(S.block(bp, "Defense").get("ArmorType") for bp in g.units().values())
    S.expect("armour_entries", len(table), 6)
    S.expect("multiplier_strings", sum(len(m) for m in table.values()), 10)
    S.expect("armour_table", table, ARMOUR)
    S.expect("ArmorType_in_use", dict(sorted(used.items())), {"Commander": 4, "Experimental": 6, "Light": 56, "Normal": 296, "Structure": 206})
    S.expect("ArmorType_not_in_table", sorted(set(used) - set(table)), [])


def damage_types(g: S.Game) -> None:
    table = {d for m in armour_table(g).values() for d in m}
    used = {w.get("DamageType") for bp in g.units().values() for w in bp.get("Weapon", S.Table()).arr if isinstance(w, S.Table)}
    S.expect("weapon_DamageType", sorted(used), ["Deathnuke", "EMP", "Normal", "Overcharge"])
    S.expect("table_damage_types", sorted(table), ["Deathnuke", "ExperimentalFootfall", "Normal", "Overcharge"])
    S.expect("used_without_table_entry", sorted(used - table), ["EMP"])
    S.expect("table_entry_without_weapon", sorted(table - used), ["ExperimentalFootfall"])


def shields(g: S.Game) -> None:
    blocks = [b for b in (S.block(bp, "Defense", "Shield") for bp in g.units().values()) if b]
    S.expect("blueprints_with_Shield", len(blocks), 21)
    for key in SHIELD_KEYS:
        S.expect(key, sum(key in b for b in blocks), 21)
    states = ("Shield", "UnitShield", "AntiArtilleryShield", "OnState", "OffState", "DamageRechargeState", "EnergyDrainRechargeState", "DeadState", "OnGetDamageAbsorption", "GetOverkill")
    S.need_defs(g, "lua", "lua/shield.lua", states, "shield_names")


if __name__ == "__main__":
    sys.exit(S.run({"SIM-022": armour, "SIM-023": damage_types, "SIM-024": shields}, sys.argv[1:]))
