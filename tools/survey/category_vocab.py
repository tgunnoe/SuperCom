#!/usr/bin/env python3
"""Print the distinct category strings of the installed unit blueprints, sorted.

Reads units.scd in memory and prints one identifier per line; nothing else.
Usage: python3 -I tools/survey/category_vocab.py <install dir>
"""

import pathlib
import re
import sys
import zipfile

CATEGORIES = re.compile(r"Categories\s*=\s*\{(.*?)\}", re.S)
STRING = re.compile(r"'([^']*)'")


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    archive = pathlib.Path(argv[1]) / "gamedata" / "units.scd"
    if not archive.is_file():
        print(f"SKIP: {archive} not found", file=sys.stderr)
        return 2
    vocabulary: set[str] = set()
    with zipfile.ZipFile(archive) as z:
        for name in z.namelist():
            if name.lower().endswith("_unit.bp"):
                text = z.read(name).decode("latin-1")
                for block in CATEGORIES.findall(text):
                    vocabulary.update(STRING.findall(block))
    for category in sorted(vocabulary):
        print(category)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
