#!/usr/bin/env python3
"""Evaluator for the dialect-scope and duplicate-key clauses of docs/specs/formats.md.

FMT-017 counts the blueprint files by constructor and the map Lua files; FMT-026 reads
every in-scope blueprint with the simlib data-subset reader, counting repeated named keys.
Both read archives in memory and print counts only.
"""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fmtlib as F


def scope(game) -> None:
    root = os.environ[F.S.FA_VAR]
    census, outside = Counter(), 0
    for _, _, ctor, text in F.blueprint_members(game, root):
        census[ctor] += 1
        if ctor in F.OUT_OF_SCOPE:
            try:
                F.S.parse_table(text, "emitter")
            except F.Fail:
                outside += 1
    F.expect("units", census["UnitBlueprint"], 568)
    F.expect("projectiles", census["ProjectileBlueprint"], 365)
    F.expect("props", census["PropBlueprint"], 336)
    F.expect("emitters", census["EmitterBlueprint"], 2437)
    F.expect("trail_emitters", census["TrailEmitterBlueprint"], 184)
    F.expect("effect_files_outside_the_subset", outside, 2621)
    F.expect("other_constructors", sum(census.values()) - sum(census[c] for c in F.IN_SCOPE + F.OUT_OF_SCOPE), 1)
    maps = os.path.join(root, "maps")
    saves = scenarios = 0
    for folder in os.listdir(maps):
        inner = os.path.join(maps, folder)
        if os.path.isdir(inner):
            names = [n.lower() for n in os.listdir(inner)]
            saves += sum(n.endswith("_save.lua") for n in names)
            scenarios += sum(n.endswith("_scenario.lua") for n in names)
    F.expect("save_files", saves, 61)
    F.expect("scenario_files", scenarios, 61)


def duplicates(game) -> None:
    root = os.environ[F.S.FA_VAR]
    files = 0
    for _, name, ctor, text in F.blueprint_members(game, root):
        if ctor not in F.IN_SCOPE:
            continue
        reader = F.DupReader(text, name)
        while reader.peek()[1] != "{":
            reader.take()
        reader.table()
        files += 1
    F.expect("files", files, 1444)
    F.show("tables", F.DupReader.tables)
    F.expect("repeated_named_keys", F.DupReader.repeats, 0)


CHECKS = {"FMT-017": scope, "FMT-026": duplicates}

if __name__ == "__main__":
    sys.exit(F.S.run(CHECKS, sys.argv[1:]))
