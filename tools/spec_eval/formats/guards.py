#!/usr/bin/env python3
"""Evaluator for the reserved-variant and no-float clauses of docs/specs/formats.md (FMT-025, FMT-045, FMT-073).

Both clauses describe code, so the script reads the repository only: it counts the
places where a crate's source builds the reserved error variant, outside the enum
definition, its Display arm, and doc comments.
"""

import os
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fmtlib as F


def constructions(source: str, variant: str) -> tuple[int, int]:
    """(places the variant is named in code, places it is built), doc comments excluded."""
    named = built = 0
    for line in source.splitlines():
        code = line.split("//", 1)[0]
        if re.search(rf"\b{variant}\b", code):
            named += 1
            use = re.search(rf"\b(?:Error|Self)::{variant}\b", code)
            arrow = code.find("=>")
            # Before an arrow the variant is a match pattern; anywhere else it is built.
            if use and (arrow == -1 or use.start() > arrow):
                built += 1
    return named, built


def reserved(path: str, variant: str):
    def check(_game) -> None:
        named, built = constructions(F.repo_text(path), variant)
        F.expect("places_naming_the_variant", named, 2)
        F.expect("places_building_the_variant", built, 0)

    return check


def integer_only(_game) -> None:
    """FMT-073: no `f32` or `f64` in the code of the texture crate's source files."""
    import glob

    found = []
    for path in sorted(glob.glob(os.path.join(F.REPO, "crates", "dds", "src", "*.rs"))):
        for number, line in enumerate(F.repo_text(os.path.relpath(path, F.REPO)).splitlines(), 1):
            if re.search(r"\bf(32|64)\b", line.split("//", 1)[0]):
                found.append(f"{os.path.basename(path)}:{number}")
    F.expect("lines_naming_a_float_type", found, [])


CHECKS = {
    "FMT-025": reserved("crates/bp/src/lib.rs", "Encoding"),
    "FMT-045": reserved("crates/dds/src/lib.rs", "UnsupportedVersion"),
    "FMT-073": integer_only,
}

if __name__ == "__main__":
    sys.exit(F.S.run(CHECKS, sys.argv[1:], offline=frozenset(CHECKS)))
