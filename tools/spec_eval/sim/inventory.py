#!/usr/bin/env python3
"""Generate the engine rows of the engine-API inventory in docs/specs/sim.md.

Not an evaluator: run it by hand and commit its output into the inventory table.
It fetches FAF's engine stubs for Unit, UnitWeapon, Projectile, Entity, and CAiBrain
at the pinned commit and prints one pipe-table row per declared function, ordered
by file and then by line: Name (Class:method), Owner (engine), Citation (the stub
URL with the line), and Detail (the declared parameter names). Nothing but names and
line numbers is kept from the stubs, which carry no licence.

Usage: python3 tools/spec_eval/sim/inventory.py > rows.md
"""

import re
import sys
import urllib.request

COMMIT = "041f661457b0"
BASE = f"https://raw.githubusercontent.com/FAForever/fa/{COMMIT}/engine/Sim/"
FILES = ("Unit", "UnitWeapon", "Projectile", "Entity", "CAiBrain")
FUNCTION = re.compile(r"^function (\w+):(\w+)\(([^)]*)\)")


def main() -> int:
    total = 0
    for stub in FILES:
        url = f"{BASE}{stub}.lua"
        with urllib.request.urlopen(url, timeout=60) as response:
            text = response.read().decode("utf-8")
        for number, line in enumerate(text.splitlines(), start=1):
            found = FUNCTION.match(line)
            if found:
                params = " ".join(p.strip() for p in found[3].split(",") if p.strip()) or "none"
                print(f"| {found[1]}:{found[2]} | engine | {url}#L{number} | {params} |")
                total += 1
    print(f"inventory.py: {total} rows", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
