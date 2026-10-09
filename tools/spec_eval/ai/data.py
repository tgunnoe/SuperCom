#!/usr/bin/env python3
"""Evaluator for the AI data clauses of docs/specs/ai.md (SKAI-005 to SKAI-008)."""

import os
import re
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ailib as A

TOP = ["AIAddBuilderTable", "AIBehaviors", "SkirmishStrategyList", "Strategy-Test", "aiarchetype-managerloader", "aiattackutilities", "aibuildstructures", "aibuildunits", "aiutilities"]
BUILDERS = ["AIAirAttackBuilders", "AIArtilleryBuilders", "AIDefenseBuilders", "AIEconomicBuilders", "AIEconomyUpgradeBuilders", "AIExpansionBuilders", "AIExperimentalBuilders", "AIFactoryConstructionBuilders", "AIIntelBuilders", "AILandAttackBuilders", "AINavalBuilders", "AISeaAttackBuilders"]
PLATOONS = {"Air": 28, "Engineer": 21, "Land": 38, "Sea": 15, "Structure": 25}


def members(g: A.Game) -> list[str]:
    return g.members("lua", "lua/AI/")


def files_in(g: A.Game, group: str) -> list[str]:
    return sorted(n.rsplit("/", 1)[-1] for n in members(g) if n.startswith(f"lua/AI/{group}/") and not n.endswith("/"))


def layout(g: A.Game) -> None:
    every = members(g)
    A.expect("holders", dict(g.holders("lua/AI/")), {"lua": 94})
    A.expect("members", len(every), 94)
    A.expect("directory_entries", sum(n.endswith("/") for n in every), 5)
    groups = Counter(n.split("/")[2] for n in every if n.count("/") >= 3)
    A.expect("group_members", dict(sorted(groups.items())), {"AIBaseTemplates": 24, "AIBuilders": 13, "OpAI": 41, "PlatoonTemplates": 6})
    sizes = {k: len(files_in(g, k)) for k in sorted(groups)}
    A.expect("group_files", sizes, {"AIBaseTemplates": 23, "AIBuilders": 12, "OpAI": 40, "PlatoonTemplates": 5})
    top = sorted(n[len("lua/AI/") : -len(".lua")] for n in every if n.count("/") == 2 and n.endswith(".lua"))
    A.expect("top_level_files", len(top), 9)
    A.expect("top_level_names", top, sorted(TOP))
    A.expect("files", sum(not n.endswith("/") for n in every), 89)


def builders(g: A.Game) -> None:
    A.expect("builder_files", [f[:-4] for f in files_in(g, "AIBuilders")], BUILDERS)
    groups = tables = named = 0
    for name in BUILDERS:
        text = A.strip_comments(g.text("lua", f"lua/AI/AIBuilders/{name}.lua"))
        groups += len(re.findall(r"\bBuilderGroup\s*\{", text))
        tables += len(re.findall(r"\bBuilder\s*\{", text))
        named += len(re.findall(r"\bBuilderName\s*=", text))
    A.expect("BuilderGroup_tables", groups, 141)
    A.expect("Builder_tables", tables, 410)
    A.expect("BuilderName_entries", named, 410)


def platoons(g: A.Game) -> None:
    A.expect("platoon_files", [f[:-4] for f in files_in(g, "PlatoonTemplates")], sorted(f"{k}PlatoonTemplates" for k in PLATOONS))
    found = {}
    for kind in PLATOONS:
        text = A.strip_comments(g.text("lua", f"lua/AI/PlatoonTemplates/{kind}PlatoonTemplates.lua"))
        found[kind] = len(re.findall(r"\bPlatoonTemplate\s*\{", text))
    A.expect("PlatoonTemplate_tables_by_file", found, PLATOONS)
    A.expect("PlatoonTemplate_tables", sum(found.values()), 127)


def categories(g: A.Game) -> None:
    used: Counter = Counter()
    for name in BUILDERS:
        text = A.strip_comments(g.text("lua", f"lua/AI/AIBuilders/{name}.lua"))
        used.update(re.findall(r"\bcategories\.([A-Za-z0-9_]+)", text))
    vocab = set().union(*(bp["cats"] for bp in g.units().values()))
    A.expect("category_references", sum(used.values()), 440)
    A.expect("distinct_categories_named", len(used), 37)
    A.expect("named_outside_the_blueprint_vocabulary", sorted(c for c in used if c not in vocab), [])
    A.expect("vocabulary", len(vocab), 152)


if __name__ == "__main__":
    sys.exit(A.run({"SKAI-005": layout, "SKAI-006": builders, "SKAI-007": platoons, "SKAI-008": categories}, sys.argv[1:]))
