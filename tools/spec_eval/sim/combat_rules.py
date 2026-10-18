#!/usr/bin/env python3
"""Evaluator for the combat game-data clauses of docs/specs/sim.md (SIM-071, SIM-072, SIM-074).

Reads the unit and projectile blueprints and the Lua members in memory and prints
identifiers, line numbers, and counts only; no line of the installed game is echoed.
"""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

# Field -> weapon entries (of 494) that carry it.
WEAPON_FIELDS = {
    "Damage": 494,
    "DamageType": 494,
    "Label": 494,
    "WeaponCategory": 494,
    "RateOfFire": 403,
    "MaxRadius": 408,
    "MinRadius": 87,
    "DamageRadius": 266,
    "MuzzleVelocity": 383,
    "ProjectileId": 382,
    "FireTargetLayerCapsTable": 395,
    "TargetPriorities": 337,
    "MuzzleSalvoSize": 402,
    "MuzzleSalvoDelay": 402,
    "RackSalvoReloadTime": 393,
    "RackSalvoChargeTime": 393,
    "ProjectilesPerOnFire": 255,
    "BallisticArc": 393,
    "ManualFire": 22,
    "EnabledByEnhancement": 6,
    "TrackingRadius": 309,
    "TargetCheckInterval": 404,
    "FiringTolerance": 457,
}
WEAPON_RANGES = {
    "RateOfFire": (0.03, 10),
    "MaxRadius": (1, 20000),
    "MinRadius": (0, 175),
    "DamageRadius": (0, 20),
    "MuzzleVelocity": (0, 160),
}
ARCS = {"RULEUBA_None": 294, "RULEUBA_LowArc": 83, "RULEUBA_HighArc": 16}
# (line, identifier the line names) of lua/sim/defaultweapons.lua of mohodata.scd.
CYCLE_LINES = (
    (541, "RateOfFire"),
    (547, "CurrentRackSalvoNumber"),
    (549, "MuzzleSalvoSize"),
    (550, "MuzzleSalvoDelay"),
    (600, "MuzzleSalvoDelay"),
    (624, "RackBones"),
    (626, "RackSalvoReloadTime"),
    (672, "RackSalvoReloadTime"),
)
PROJECTILE_FIELDS = {
    "InitialSpeed": 234,
    "MaxSpeed": 132,
    "Acceleration": 123,
    "TurnRate": 193,
    "Lifetime": 84,
    "TrackTarget": 128,
    "UseGravity": 152,
    "DestroyOnWater": 286,
    "LeadTarget": 109,
    "VelocityAlign": 259,
}
PROJECTILE_RANGES = {
    "InitialSpeed": (0, 120),
    "MaxSpeed": (0, 600),
    "Acceleration": (0, 600),
    "TurnRate": (0, 1000),
    "Lifetime": (0.5, 1000),
}
# (line, identifier) of lua/sim/Unit.lua of lua.scd.
DAMAGE_LINES = (
    (794, "DoTakeDamage"),
    (796, "AdjustHealth"),
    (798, "health"),
    (799, "Reclaimed"),
    (804, "excess"),
    (807, "excessDamageRatio"),
    (809, "Kill"),
)
ARMOUR_NAMES = ["Commander", "Default", "Experimental", "Light", "Normal", "Structure"]


def weapons_of(g: S.Game) -> list[S.Table]:
    return [w for bp in g.units().values() for w in bp.get("Weapon", S.Table()).arr if isinstance(w, S.Table)]


def numbers(tables, field) -> tuple:
    values = [t[field] for t in tables if isinstance(t.get(field), (int, float)) and not isinstance(t.get(field), bool)]
    return (min(values), max(values))


def line_names(g: S.Game, archive: str, member: str, lines) -> None:
    winner = g.index().get(member.lower())
    if winner != archive:
        raise S.Fail(f"{member} is won by {winner}, not {archive}")
    text = g.text(archive, member).splitlines()
    for number, name in lines:
        if name not in text[number - 1]:
            raise S.Fail(f"line {number} of {member} does not name {name}")
    S.show(f"{member}_lines_checked", [n for n, _ in lines])


def weapon_census(g: S.Game) -> None:
    ws = weapons_of(g)
    S.expect("weapon_entries", len(ws), 494)
    S.expect("units_with_Weapon", sum(isinstance(bp.get("Weapon"), S.Table) for bp in g.units().values()), 247)
    have = {f: sum(f in w for w in ws) for f in WEAPON_FIELDS}
    S.expect("weapon_field_presence", have, WEAPON_FIELDS)
    for field, want in WEAPON_RANGES.items():
        S.expect(f"{field}_range", numbers([w for w in ws], field), want)
    S.expect("RateOfFire_zero", sum(w.get("RateOfFire") == 0 for w in ws), 0)
    S.expect("BallisticArc", dict(Counter(w["BallisticArc"] for w in ws if "BallisticArc" in w)), ARCS)
    enhanced = [w for w in ws if "EnabledByEnhancement" in w]
    S.expect("EnabledByEnhancement_manual", sum(bool(w.get("ManualFire")) for w in enhanced), 2)
    owners = sum(any(isinstance(w, S.Table) and "EnabledByEnhancement" in w for w in bp.get("Weapon", S.Table()).arr) for bp in g.units().values())
    S.expect("EnabledByEnhancement_units", owners, 4)
    S.expect("without_speed_or_projectile", sum(not (w.get("MuzzleVelocity", 0) > 0) and not w.get("ProjectileId") for w in ws), 111)
    line_names(g, "mohodata", "lua/sim/defaultweapons.lua", CYCLE_LINES)


def projectile_blocks(g: S.Game) -> list[S.Table]:
    names = [n for n in g.names("projectiles") if n.lower().endswith("_proj.bp")]
    return [S.block(S.parse_table(g.text("projectiles", n), n), "Physics") for n in names]


def projectile_defense_census(g: S.Game) -> None:
    blocks = projectile_blocks(g)
    S.expect("projectile_blueprints", len(blocks), 287)
    S.expect("physics_field_presence", {f: sum(f in b for b in blocks) for f in PROJECTILE_FIELDS}, PROJECTILE_FIELDS)
    for field, want in PROJECTILE_RANGES.items():
        S.expect(f"{field}_range", numbers(blocks, field), want)
    defense = [S.block(bp, "Defense") for bp in g.units().values()]
    for field in ("Health", "MaxHealth", "ArmorType"):
        S.expect(f"Defense_{field}", sum(field in d for d in defense), 568)
    regen = Counter(d["RegenRate"] for d in defense if "RegenRate" in d)
    S.expect("Defense_RegenRate", sum(regen.values()), 563)
    S.expect("RegenRate_zero", regen[0], 547)
    S.expect("RegenRate_nonzero_range", (min(k for k in regen if k), max(regen)), (0.5, 25))


def damage_path(g: S.Game) -> None:
    line_names(g, "lua", "lua/sim/Unit.lua", DAMAGE_LINES)
    shadowed = sorted(a for a in ("lua", "mohodata") if "lua/sim/unit.lua" in {n.lower() for n in g.names(a)})
    S.expect("Unit_lua_archives", shadowed, ["lua", "mohodata"])
    winner = g.index().get("lua/armordefinition.lua")
    S.expect("armordefinition_winner", winner, "lua")
    text = g.text("lua", "lua/armordefinition.lua")
    entries = S.parse_assignments(text, "lua/armordefinition.lua")["armordefinition"].arr
    S.expect("armour_names", sorted(e.arr[0] for e in entries), ARMOUR_NAMES)
    strings = [s for e in entries for s in e.arr[1:]]
    S.expect("multiplier_strings", len(strings), 10)
    pairs = sorted((e.arr[0], k, float(v)) for e in entries for k, v in (s.split() for s in e.arr[1:]))
    S.expect(
        "multipliers",
        pairs,
        [
            ("Commander", "Deathnuke", 0.05),
            ("Commander", "Normal", 1.0),
            ("Commander", "Overcharge", 0.033333),
            ("Default", "Normal", 1.0),
            ("Experimental", "ExperimentalFootfall", 0.0),
            ("Light", "Normal", 1.0),
            ("Normal", "Normal", 1.0),
            ("Structure", "Deathnuke", 0.01),
            ("Structure", "Normal", 1.0),
            ("Structure", "Overcharge", 0.066666),
        ],
    )


if __name__ == "__main__":
    sys.exit(S.run({"SIM-071": weapon_census, "SIM-072": projectile_defense_census, "SIM-074": damage_path}, sys.argv[1:]))
