#!/usr/bin/env python3
"""Census of unit blueprint structure in units.scd. Read-only.

Usage: blueprint_census.py [FA_DIR]   (default: $SUPERCOM_FA_DIR)
Prints counts only (blueprints with an Intel block; distinct category
strings); no blueprint content is printed.
"""

import os
import re
import sys
import zipfile

CATEGORIES = re.compile(r"Categories\s*=\s*\{(.*?)\}", re.S)
INTEL = re.compile(r"^\s*Intel\s*=\s*\{", re.M)
TOKEN = re.compile(r"'([A-Z0-9_]+)'")


def main() -> int:
    fa = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("SUPERCOM_FA_DIR", "")
    units = with_intel = 0
    categories = set()
    with zipfile.ZipFile(os.path.join(fa, "gamedata", "units.scd")) as archive:
        for info in archive.infolist():
            if not info.filename.lower().endswith("_unit.bp"):
                continue
            units += 1
            text = archive.read(info).decode("utf-8", "ignore")
            if INTEL.search(text):
                with_intel += 1
            match = CATEGORIES.search(text)
            if match:
                categories.update(TOKEN.findall(match.group(1)))
    print(f"unit_blueprints {units} with_intel {with_intel} distinct_categories {len(categories)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
