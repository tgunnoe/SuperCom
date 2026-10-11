#!/usr/bin/env python3
"""Evaluator for the archive-layer clauses of docs/specs/formats.md (FMT-005, FMT-007)."""

import os
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fmtlib as F

SHADOWED = {
    "lua/sim/projectile.lua": ["lua", "mohodata"],
    "lua/sim/prop.lua": ["lua", "mohodata"],
    "lua/sim/unit.lua": ["lua", "mohodata"],
    "lua/simutils.lua": ["lua", "mohodata"],
}


def mount_points(game) -> None:
    root = os.environ[F.S.FA_VAR]
    path = os.path.join(root, "bin", "SupComDataPath.lua")
    try:
        with open(path, "rb") as handle:
            lines = handle.read().decode("latin-1").splitlines()
    except OSError:
        raise F.Fail("missing install file bin/SupComDataPath.lua") from None
    calls = [
        (number, match.group(1), match.group(2))
        for number, line in enumerate(lines, 1)
        if (match := re.match(r"(mount_contents|mount_dir)\((.*)\)\s*$", line))
    ]
    F.expect("top_level_mount_calls", len(calls), 4)
    F.expect("call_lines", [c[0] for c in calls], [20, 21, 22, 23])
    F.expect("call_kinds", [c[1] for c in calls], ["mount_contents", "mount_contents", "mount_dir", "mount_dir"])
    if not all("PERSONAL" in c[2] for c in calls[:2]):
        raise F.Fail("the first two mount calls are not user-directory mounts")
    if "gamedata" not in calls[2][2] or "*.scd" not in calls[2][2]:
        raise F.Fail("the third mount call is not the gamedata *.scd glob")
    if re.search(r"InitFileDir\s*\.\.\s*'\\\\\.\.'", calls[3][2]) is None:
        raise F.Fail("the fourth mount call is not the install root")
    F.expect("archive_glob_occurrences", sum("*.scd" in line for line in lines), 1)


def shadowed(game) -> None:
    root = os.environ[F.S.FA_VAR]
    held = F.holders(game, root)
    shared = {path: pairs for path, pairs in held.items() if len(pairs) > 1}
    F.expect("archives", len(F.archive_names(root)), 17)
    F.expect("shadowed_paths", len(shared), 4)
    if {path: [stem for stem, _ in pairs] for path, pairs in shared.items()} != SHADOWED:
        raise F.Fail(f"shadowed paths differ: {sorted(shared)}")
    for path, pairs in shared.items():
        copies = {game.raw(stem, name) for stem, name in pairs}
        if len(copies) != len(pairs):
            raise F.Fail(f"the copies of {path} do not differ")
    F.show("differing_copies", len(shared))


CHECKS = {"FMT-005": mount_points, "FMT-007": shadowed}

if __name__ == "__main__":
    sys.exit(F.S.run(CHECKS, sys.argv[1:]))
