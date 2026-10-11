#!/usr/bin/env python3
"""Evaluator for the importer's archive-set clause of docs/specs/data-model.md (DAT-016).

Reads the importer's source for its archive list and the installed game for the paths
that two archives hold. Prints counts only.
"""

import os
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "formats"))
import fmtlib as F

SIX = ["units.scd", "projectiles.scd", "effects.scd", "env.scd", "meshes.scd", "props.scd"]


def archive_set(game) -> None:
    source = F.repo_text("crates/bp-import/src/lib.rs")
    listed = re.search(r"const ARCHIVES: \[&str; (\d+)\] = \[(.*?)\];", source, re.S)
    if listed is None:
        raise F.Fail("crates/bp-import/src/lib.rs has no ARCHIVES constant")
    F.expect("declared_length", int(listed.group(1)), 6)
    F.expect("archives", re.findall(r'"([^"]+)"', listed.group(2)), SIX)
    F.expect("uses_the_vfs", "Vfs" in source, False)
    root = os.environ[F.S.FA_VAR]
    stems = F.archive_names(root)
    missing = [name for name in SIX if name[:-4] not in stems]
    if missing:
        raise F.Fail(f"the install lacks gamedata/{missing[0]}")
    shared = [p for p, pairs in F.holders(game, root).items() if len(pairs) > 1]
    F.expect("shared_paths", len(shared), 4)
    F.expect("shared_blueprint_paths", sum(p.endswith(".bp") for p in shared), 0)
    F.expect("in_byte_order", SIX == sorted(SIX), False)


CHECKS = {"DAT-016": archive_set}

if __name__ == "__main__":
    sys.exit(F.S.run(CHECKS, sys.argv[1:]))
