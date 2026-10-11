#!/usr/bin/env python3
"""Evaluator for the weapon and projectile clauses of docs/specs/sim.md (SIM-019 to SIM-021)."""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

CATEGORIES = {"Anti Air": 112, "Anti Navy": 42, "Artillery": 21, "Bomb": 11, "Death": 99, "Defense": 43, "Direct Fire": 91, "Direct Fire Experimental": 7, "Direct Fire Naval": 34, "Experimental": 2, "Kamikaze": 2, "Missile": 30}
DAMAGE_TYPES = {"Deathnuke": 4, "EMP": 1, "Normal": 484, "Overcharge": 5}


def weapons_of(g: S.Game) -> list[S.Table]:
    return [w for bp in g.units().values() for w in bp.get("Weapon", S.Table()).arr if isinstance(w, S.Table)]


def weapon_vocabulary(g: S.Game) -> None:
    units = g.units()
    ws = weapons_of(g)
    S.expect("units_with_Weapon", sum(1 for bp in units.values() if isinstance(bp.get("Weapon"), S.Table)), 247)
    S.expect("weapon_entries", len(ws), 494)
    S.expect("WeaponCategory", dict(sorted(Counter(w.get("WeaponCategory") for w in ws).items())), CATEGORIES)
    S.expect("DamageType", dict(sorted(Counter(w.get("DamageType") for w in ws).items())), DAMAGE_TYPES)


def projectile_links(g: S.Game) -> None:
    ws = weapons_of(g)
    ids = {w["ProjectileId"] for w in ws if w.get("ProjectileId")}
    known = g.index()
    names = [n for n in g.names("projectiles") if n.lower().endswith("_proj.bp")]
    blocks = [S.block(S.parse_table(g.text("projectiles", n), n), "Physics") for n in names]
    S.expect("weapons_without_ProjectileId", sum(1 for w in ws if not w.get("ProjectileId")), 112)
    S.expect("distinct_ProjectileId", len(ids), 218)
    S.expect("ProjectileId_unresolved", sorted(i for i in ids if i.lstrip("/").lower() not in known), [])
    S.expect("projectile_blueprints_in_projectiles_scd", len(names), 287)
    S.expect("with_Physics", sum(1 for b in blocks if b), 287)
    S.expect("with_DestroyOnWater", sum("DestroyOnWater" in b for b in blocks), 286)


def fire_sequence(g: S.Game) -> None:
    states = ("DefaultProjectileWeapon", "IdleState", "RackSalvoChargeState", "RackSalvoFireReadyState", "RackSalvoFiringState", "RackSalvoReloadState", "WeaponUnpackingState", "WeaponPackingState", "DeadState", "KamikazeWeapon", "BareBonesWeapon", "DefaultBeamWeapon")
    S.need_defs(g, "mohodata", "lua/sim/defaultweapons.lua", states, "defaultweapons_names")
    base = ("OnFire", "OnGotTarget", "OnLostTarget", "OnEnableWeapon", "GetDamageTable", "CreateProjectileForWeapon", "AddDamageMod", "OnVeteranLevel")
    S.need_defs(g, "mohodata", "lua/sim/weapon.lua", base, "weapon_names")


if __name__ == "__main__":
    sys.exit(S.run({"SIM-019": weapon_vocabulary, "SIM-020": projectile_links, "SIM-021": fire_sequence}, sys.argv[1:]))
