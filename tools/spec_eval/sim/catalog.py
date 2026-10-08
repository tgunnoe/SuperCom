#!/usr/bin/env python3
"""Evaluator for the clause-set and inventory clauses of docs/specs/sim.md (SIM-037 to SIM-039).

SIM-037 and SIM-038 read only the repository, so they run without an install. SIM-039
checks the Lua-side behaviour groups of the inventory against the installed game.
"""

import os
import re
import sys
from collections import Counter
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

SPEC = Path(__file__).resolve().parents[3] / "docs" / "specs" / "sim.md"
PER_CLASS = {"CAiBrain": 68, "Entity": 64, "Projectile": 35, "Unit": 127, "UnitWeapon": 32}
HEADER = re.compile(r"^\*\*(SIM-\d{3})\*\* `([\w-]+)`")
STUB = "https://raw.githubusercontent.com/FAForever/fa/041f661457b0/engine/Sim/"


def spec_lines() -> list[str]:
    try:
        return SPEC.read_text(encoding="utf-8").splitlines()
    except OSError:
        raise S.Fail("missing docs/specs/sim.md") from None


def inventory_rows() -> list[list[str]]:
    rows = []
    for line in spec_lines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")] if line.startswith("|") else []
        if len(cells) == 4 and cells[1] in ("engine", "lua"):
            rows.append(cells)
    return rows


def derived_premises(_game) -> None:
    clauses: dict[str, list[str]] = {}
    current = None
    for line in spec_lines():
        found = HEADER.match(line)
        if found:
            current = found[1]
            clauses[current] = [found[2], ""]
        elif not line.strip():
            current = None
        elif current:
            clauses[current][1] += " " + line
    derived = {i: v for i, v in clauses.items() if v[0] == "derived"}
    S.expect("clauses", len(clauses), 39)
    S.expect("derived_clauses_present", bool(derived), True)
    for ident, (_, text) in derived.items():
        cited = set(re.findall(r"SIM-\d{3}", text)) - {ident}
        if not cited:
            raise S.Fail(f"{ident} is derived but names no premise")
        missing = sorted(cited - set(clauses))
        if missing:
            raise S.Fail(f"{ident} names {' '.join(missing)}, which sim.md lacks")


def engine_rows(_game) -> None:
    rows = [r for r in inventory_rows() if r[1] == "engine"]
    per_class = Counter(r[0].split(":")[0] for r in rows)
    S.expect("engine_rows", len(rows), 326)
    S.expect("engine_rows_per_class", dict(sorted(per_class.items())), PER_CLASS)
    names = Counter(r[0] for r in rows)
    S.expect("distinct_engine_names", len(names), 323)
    S.expect("names_declared_twice", sorted(n for n, c in names.items() if c > 1), ["CAiBrain:GetAvailableFactories", "CAiBrain:GetPlatoonUniquelyNamed", "CAiBrain:PickBestAttackVector"])
    last: dict[str, int] = {}
    for name, _, cite, _ in rows:
        cls = name.split(":")[0]
        found = re.fullmatch(re.escape(STUB + cls + ".lua") + r"#L(\d+)", cite)
        if not found:
            raise S.Fail(f"{name} cites {cite}, not the {cls} stub at the pinned commit")
        if int(found[1]) <= last.get(cls, 0):
            raise S.Fail(f"{name} is out of line order in the {cls} rows")
        last[cls] = int(found[1])


def lua_rows(game: S.Game) -> None:
    rows = [r for r in inventory_rows() if r[1] == "lua"]
    S.expect("lua_groups_within_12_to_60", 12 <= len(rows) <= 60, True)
    S.show("lua_groups", len(rows))
    named = 0
    for name, _, cite, detail in rows:
        found = re.fullmatch(r"fa:gamedata/(\w+)\.scd!(\S+)", cite)
        if not found:
            raise S.Fail(f"group {name!r} cites {cite!r}, not one fa:gamedata/<archive>.scd!<member> locator")
        archive, member = found[1], found[2]
        if game.index().get(member.lower()) != archive:
            raise S.Fail(f"group {name!r}: {member} is not won by {archive}")
        text = game.text(archive, member)
        defs = S.lua_defs(text)
        for ident in detail.split():
            pattern = rf"^\s*(?:local\s+)?{re.escape(ident)}\s*=|\b{re.escape(ident)}\s*[({{]"
            if ident not in defs and not re.search(pattern, text, re.M):
                raise S.Fail(f"group {name!r}: gamedata/{archive}.scd!{member} does not define {ident}")
            named += 1
    S.show("identifiers_checked", named)


if __name__ == "__main__":
    checks = {"SIM-037": derived_premises, "SIM-038": engine_rows, "SIM-039": lua_rows}
    sys.exit(S.run(checks, sys.argv[1:], offline=frozenset({"SIM-037", "SIM-038"})))
