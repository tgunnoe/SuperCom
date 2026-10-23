#!/usr/bin/env python3
"""Evaluator for the economy and build game-data clauses of docs/specs/sim.md (SIM-052 to SIM-055).

Reads lua.scd, mohodata.scd, the unit blueprints, and the installed save files in memory or
read-only, and prints counts, line numbers, and identifiers only; a line of the installed Lua
is checked for the identifiers and literals this file names and is never echoed.

Shadowed Lua (AGENTS.md U9): lua/sim/Unit.lua is one of the four members that lua.scd and
mohodata.scd both hold. The mount order picks lua.scd's copy, so SIM-052 and SIM-053 read
lua.scd's copy of it and depend on that precedence; the script looks members up
case-insensitively and reports the member as the archive names it.
"""

import glob
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build as B
import simlib as S

# (member, line, identifiers and literals the line must carry)
FORMULA = [
    ("lua/game.lua", 24, ["GetConstructEconomyModel"]),
    ("lua/game.lua", 29, ["GetBuildRate"]),
    ("lua/game.lua", 31, ["BuildTime"]),
    ("lua/game.lua", 32, ["BuildCostMass"]),
    ("lua/game.lua", 33, ["BuildCostEnergy"]),
    ("lua/game.lua", 36, ["BuildTimeModifier", "or 0"]),
    ("lua/game.lua", 37, ["100", "time_mod", ".01"]),
    ("lua/game.lua", 38, ["time<.1", ".1"]),
    ("lua/game.lua", 41, ["EnergyModifier", "or 0"]),
    ("lua/game.lua", 42, ["100", "energy_mod", ".01"]),
    ("lua/game.lua", 43, ["energy<0", "= 0"]),
    ("lua/game.lua", 46, ["MassModifier", "or 0"]),
    ("lua/game.lua", 47, ["100", "mass_mod", ".01"]),
    ("lua/game.lua", 48, ["mass<0", "= 0"]),
    ("lua/game.lua", 50, ["time/rate", "energy", "mass"]),
    ("lua/sim/Unit.lua", 703, ["GetConstructEconomyModel", "WorkItem"]),
    ("lua/sim/Unit.lua", 714, ["GetBuildCosts"]),
    ("lua/sim/Unit.lua", 716, ["EnergyBuildAdjMod", "or 1"]),
    ("lua/sim/Unit.lua", 717, ["energy < 1"]),
    ("lua/sim/Unit.lua", 718, ["energy = 1"]),
    ("lua/sim/Unit.lua", 720, ["MassBuildAdjMod", "or 1"]),
    ("lua/sim/Unit.lua", 721, ["mass < 1"]),
    ("lua/sim/Unit.lua", 722, ["mass = 1"]),
    ("lua/sim/Unit.lua", 725, ["energy_rate", "energy / time"]),
    ("lua/sim/Unit.lua", 726, ["mass_rate", "mass / time"]),
    ("lua/sim/Unit.lua", 730, ["MaintenanceConsumptionPerSecondEnergy"]),
    ("lua/sim/Unit.lua", 731, ["MaintenanceConsumptionPerSecondMass"]),
    ("lua/sim/Unit.lua", 742, ["math.max", "MinConsumptionPerSecondEnergy", "or 0"]),
    ("lua/sim/Unit.lua", 743, ["math.max", "MinConsumptionPerSecondMass", "or 0"]),
    ("lua/sim/Unit.lua", 745, ["SetConsumptionPerSecondEnergy"]),
    ("lua/sim/Unit.lua", 746, ["SetConsumptionPerSecondMass"]),
]

PRODUCTION = [
    ("lua/sim/Unit.lua", 168, ["SetConsumptionPerSecondEnergy", "MaintenanceConsumptionPerSecondEnergy", "or 0"]),
    ("lua/sim/Unit.lua", 169, ["SetConsumptionPerSecondMass", "MaintenanceConsumptionPerSecondMass", "or 0"]),
    ("lua/sim/Unit.lua", 170, ["SetProductionPerSecondEnergy", "ProductionPerSecondEnergy", "or 0"]),
    ("lua/sim/Unit.lua", 171, ["SetProductionPerSecondMass", "ProductionPerSecondMass", "or 0"]),
    ("lua/sim/Unit.lua", 73, ["EconomyProductionInitiallyActive", "true"]),
    ("lua/sim/Unit.lua", 173, ["EconomyProductionInitiallyActive"]),
    ("lua/sim/Unit.lua", 175, ["SetProductionActive"]),
    ("lua/sim/Unit.lua", 758, ["SetProductionPerSecondEnergy", "EnergyProdAdjMod", "or 1"]),
    ("lua/sim/Unit.lua", 759, ["SetProductionPerSecondMass", "MassProdAdjMod", "or 1"]),
    ("lua/defaultunits.lua", 820, ["GetResourceConsumed"]),
    ("lua/defaultunits.lua", 828, ["SetProductionPerSecondMass", "massProduction", "GetResourceConsumed"]),
    ("lua/defaultunits.lua", 830, ["SetProductionPerSecondMass", "(0)"]),
    ("lua/defaultunits.lua", 866, ["MassFabricationUnit"]),
    ("lua/defaultunits.lua", 869, ["OnStopBeingBuilt"]),
    ("lua/defaultunits.lua", 871, ["SetMaintenanceConsumptionActive"]),
    ("lua/defaultunits.lua", 872, ["SetProductionActive", "true"]),
]
FAF_ONLY = ["OnExcessEnergy", "OnNoExcessEnergy", "EnergyExcess"]
EPIA = "EconomyProductionInitiallyActive"

EXTRACTOR = [
    ("lua/defaultunits.lua", 771, ["MassCollectionUnit"]),
    ("lua/defaultunits.lua", 774, ["OnCreate"]),
    ("lua/defaultunits.lua", 776, ["GetMarkers"]),
    ("lua/defaultunits.lua", 780, ["type", "MASS"]),
    ("lua/defaultunits.lua", 782, ["massPosition[1]", "unitPosition[1] + 1", "unitPosition[1] - 1"]),
    ("lua/defaultunits.lua", 783, ["massPosition[2]", "unitPosition[2] + 1", "unitPosition[2] - 1"]),
    ("lua/defaultunits.lua", 784, ["massPosition[3]", "unitPosition[3] + 1", "unitPosition[3] - 1"]),
    ("lua/defaultunits.lua", 785, ["SetProductionPerSecondMass", "amount", "100"]),
    ("lua/MarkerTemplates.lua", 9, ["Mass"]),
    ("lua/MarkerTemplates.lua", 11, ["amount = 100"]),
    ("lua/MarkerTemplates.lua", 12, ["size = 1"]),
    ("lua/MarkerTemplates.lua", 17, ["Hydrocarbon"]),
    ("lua/MarkerTemplates.lua", 19, ["amount = 100"]),
    ("lua/MarkerTemplates.lua", 20, ["size = 3"]),
]

MAX_BUILD_DISTANCE = {"UAL0001": 10, "UEL0001": 10, "UEL0105": 5, "URA0001": 100, "URL0001": 10, "XRB0104": 15, "XRB0204": 20, "XRB0304": 25, "XRL0403": 1, "XSL0001": 10}
CATEGORY_COUNTS = {"FACTORY": 51, "ENGINEER": 28, "MASSEXTRACTION": 12, "HYDROCARBON": 4, "MASSFABRICATION": 14, "ENERGYPRODUCTION": 22, "MASSSTORAGE": 4, "ENERGYSTORAGE": 4}
SIZE_X = {0: 1, 1: 92, 2: 39, 3: 164, 4: 1, 5: 52, 6: 4, 7: 6, 13: 1, 20: 3}


def member_lines(g: S.Game, archive: str, member: str) -> list[str]:
    """The member's lines, looked up case-insensitively, after checking the mount winner."""
    if g.index().get(member.lower()) != archive:
        raise S.Fail(f"{member} is not won by {archive}")
    named = {n.lower(): n for n in g.names(archive)}
    return g.text(archive, named[member.lower()]).split("\n")


def check_lines(g: S.Game, rows: list, label: str) -> None:
    cache: dict[str, list[str]] = {}
    for member, number, needles in rows:
        lines = cache.setdefault(member, member_lines(g, "lua", member))
        line = lines[number - 1] if 0 < number <= len(lines) else ""
        missing = [n for n in needles if n not in line]
        if missing:
            raise S.Fail(f"{member} line {number} lacks {' '.join(missing)}")
    S.show(f"{label}_lines_checked", len(rows))
    S.show(f"{label}_members", " ".join(sorted(cache)))


def shadowed(g: S.Game, member: str) -> None:
    holds = [a for a in ("lua", "mohodata") if member.lower() in {n.lower() for n in g.names(a)}]
    S.expect(f"{member}_held_by", holds, ["lua", "mohodata"])
    S.expect(f"{member}_won_by", g.index().get(member.lower()), "lua")


def formula(g: S.Game) -> None:
    check_lines(g, FORMULA, "formula")
    shadowed(g, "lua/sim/Unit.lua")
    game_lines = member_lines(g, "lua", "lua/game.lua")[23:50]
    S.expect("math.max_in_game.lua_24_to_50", sum("math.max" in line for line in game_lines), 0)


def production(g: S.Game) -> None:
    check_lines(g, PRODUCTION, "production")
    shadowed(g, "lua/sim/Unit.lua")
    for needle in FAF_ONLY:
        S.expect(f"{needle}_members_in_lua.scd", len(S.member_hits(g, "lua", [needle])[needle]), 0)
    found = {}
    for archive in ("lua", "mohodata"):
        for name in g.names(archive):
            if name.lower().endswith(".lua"):
                for number, line in enumerate(g.text(archive, name).split("\n"), 1):
                    if EPIA in line:
                        found.setdefault(f"{archive}:{name}", []).append((number, "= true" in line))
    S.expect(f"{EPIA}_occurrences", found, {"lua:lua/sim/Unit.lua": [(73, True), (173, False)]})


def save_markers() -> dict[str, list]:
    root = S.install_dir() or ""
    files = sorted(os.path.join(root, "maps", folder, name) for folder in os.listdir(os.path.join(root, "maps")) for name in os.listdir(os.path.join(root, "maps", folder)) if name.lower().endswith("_save.lua"))
    found: dict[str, list] = {"Mass": [], "Hydrocarbon": []}
    for path in files:
        with open(path, "rb") as handle:
            table = S.parse_assignments(S.decode(handle.read()), os.path.basename(path))
        chains = table["Scenario"]["MasterChain"]
        for chain in chains.values():
            for marker in chain.get("Markers", {}).values():
                if isinstance(marker, dict) and marker.get("type") in found:
                    found[marker["type"]].append(marker.get("amount"))
    found["files"] = files
    return found


def extractor(g: S.Game) -> None:
    check_lines(g, EXTRACTOR, "extractor")
    found = save_markers()
    S.expect("save_files", len(found["files"]), 61)
    S.expect("Mass_markers", len(found["Mass"]), 3508)
    S.expect("Hydrocarbon_markers", len(found["Hydrocarbon"]), 193)
    S.expect("distinct_marker_amounts", sorted({float(a) for a in found["Mass"] + found["Hydrocarbon"]}), [100.0])


def census(g: S.Game) -> None:
    units = g.units()
    econ = {u: S.block(bp, "Economy") for u, bp in units.items()}
    S.expect("unit_blueprints", len(units), 568)
    S.expect("MaxBuildDistance", {u: e["MaxBuildDistance"] for u, e in sorted(econ.items()) if "MaxBuildDistance" in e}, MAX_BUILD_DISTANCE)
    S.expect("builders_without_MaxBuildDistance", sum("BuildRate" in e and "MaxBuildDistance" not in e for e in econ.values()), 118)
    ground = [S.block(bp, "Physics").get("MaxGroundVariation") for bp in units.values()]
    S.expect("MaxGroundVariation_values", sorted(v for v in ground if v is not None), [50] * 15)
    footprints = [S.block(bp, "Footprint") for bp in units.values()]
    S.expect("Footprint_blocks", sum(bool(f) for f in footprints), 402)
    sizes = [f["SizeX"] for f in footprints if "SizeX" in f]
    S.expect("Footprint_blocks_with_SizeX", len(sizes), 363)
    S.expect("Footprint_blocks_without_SizeX", sum(bool(f) and "SizeX" not in f for f in footprints), 39)
    S.expect("SizeX_census", {k: sizes.count(k) for k in sorted(set(sizes))}, SIZE_X)
    for key in ("MinConsumptionPerSecondEnergy", "MinConsumptionPerSecondMass"):
        S.expect(key, sum(key in e for e in econ.values()), 0)
    S.expect("NaturalProducer", sum("NaturalProducer" in e for e in econ.values()), 10)
    missing = sorted(u for u, e in econ.items() if "BuildTime" not in e)
    S.expect("BuildTime_absent", len(missing), 16)
    selected = [u for u in missing if any(all(t in S.categories(units[u]) or t == u.lower() for t in expr.split()) for exprs in B.expressions(g).values() for expr in exprs)]
    S.expect("BuildTime_absent_and_selected_by_a_builder", selected, [])
    cats = {k: sum(k in S.categories(bp) for bp in units.values()) for k in CATEGORY_COUNTS}
    S.expect("category_counts", cats, CATEGORY_COUNTS)


if __name__ == "__main__":
    checks = {"SIM-052": formula, "SIM-053": production, "SIM-054": extractor, "SIM-055": census}
    sys.exit(S.run(checks, sys.argv[1:]))
