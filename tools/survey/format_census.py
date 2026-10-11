#!/usr/bin/env python3
"""Census of the installed game's binary formats and blueprint families.

Reads archive members in memory and prints counts, constants, and identifiers
only. Usage: python3 -I tools/survey/format_census.py <install dir>
"""

import collections
import pathlib
import re
import struct
import sys
import zipfile


def members(gamedata: pathlib.Path, suffix: str):
    for archive in sorted(gamedata.glob("*.scd")):
        with zipfile.ZipFile(archive) as z:
            for name in z.namelist():
                if name.lower().endswith(suffix):
                    yield name, z.read(name)


def scm(gamedata: pathlib.Path) -> None:
    fits = misfit = 0
    versions: collections.Counter = collections.Counter()
    first_misfit = None
    for name, b in members(gamedata, ".scm"):
        if len(b) < 48:
            misfit += 1
            continue
        (_magic, ver, bone_off, bone_n, vert_off, _extra_off, vert_n, idx_off,
         idx_n, _info_off, _info_n, total_bones) = struct.unpack_from("<4s11I", b, 0)
        versions[ver] += 1
        ok = (ver == 5 and bone_n <= total_bones
              and bone_off + total_bones * 108 <= len(b)
              and vert_off + vert_n * 68 <= len(b)
              and idx_off + idx_n * 2 <= len(b) and idx_n % 3 == 0)
        if ok:
            fits += 1
        else:
            misfit += 1
            first_misfit = first_misfit or (name, ver, len(b))
    print("scm layout_fits", fits, "misfit", misfit, "versions", dict(versions),
          "first_misfit", first_misfit)


def sca(gamedata: pathlib.Path) -> None:
    heads: collections.Counter = collections.Counter()
    for _name, b in members(gamedata, ".sca"):
        magic, ver = struct.unpack_from("<4sI", b, 0)
        heads[(magic.decode("ascii", "replace"), ver)] += 1
    print("sca", dict(heads))


def dds(gamedata: pathlib.Path) -> None:
    fourcc: collections.Counter = collections.Counter()
    bad = 0
    for _name, b in members(gamedata, ".dds"):
        if len(b) < 128 or b[:4] != b"DDS ":
            bad += 1
            continue
        (pf_flags,) = struct.unpack_from("<I", b, 80)
        kind = b[84:88].decode("ascii", "replace") if pf_flags & 0x4 else "uncompressed"
        fourcc[kind] += 1
    total = sum(fourcc.values())
    print("dds", total, "bad_header", bad,
          {k: f"{v} ({100 * v / total:.1f}%)" for k, v in fourcc.most_common()})


def scmap(install: pathlib.Path) -> None:
    heads: collections.Counter = collections.Counter()
    sizes = set()
    for path in sorted((install / "maps").glob("*/*.scmap")):
        magic, major, _u1, _u2, w, h = struct.unpack_from("<iiiiff", path.read_bytes()[:24], 0)
        heads[(hex(magic & 0xFFFFFFFF), major)] += 1
        sizes.add((int(w), int(h)))
    print("scmap", dict(heads), "sizes", sorted(sizes))


def blueprints(gamedata: pathlib.Path) -> None:
    constructors: collections.Counter = collections.Counter()
    keys: collections.Counter = collections.Counter()
    features: collections.Counter = collections.Counter()
    units = 0
    for name, b in members(gamedata, ".bp"):
        head = b[:200].decode("latin-1").lstrip()
        m = re.match(r"(\w+)\s*\{", head)
        constructors[m.group(1) if m else "other"] += 1
    with zipfile.ZipFile(gamedata / "units.scd") as z:
        unit_members = [(n, z.read(n)) for n in z.namelist() if n.lower().endswith("_unit.bp")]
    for _name, b in unit_members:
        units += 1
        text = b.decode("latin-1")
        for key in re.findall(r"^\s{4}(\w+)\s*=", text, re.M):
            keys[key] += 1
        features["LOC_strings"] += "<LOC " in text
        features["hash_comments"] += bool(re.search(r"^\s*#", text, re.M))
        features["Sound_constructor"] += bool(re.search(r"Sound\s*\{", text))
    always = sorted(k for k, c in keys.items() if c == units)
    print("bp_constructors", dict(constructors.most_common()))
    print("unit_bps_in_units_scd", units, "keys_in_every_unit", always, "features", dict(features))
    print("key_presence", {k: keys[k] for k in ("Display", "Intel", "Weapon", "Audio",
                                                "Interface", "Veteran", "Wreckage", "Footprint", "AI")})


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    install = pathlib.Path(argv[1])
    gamedata = install / "gamedata"
    if not gamedata.is_dir():
        print(f"SKIP: no gamedata directory under {install}", file=sys.stderr)
        return 2
    scm(gamedata)
    sca(gamedata)
    dds(gamedata)
    scmap(install)
    blueprints(gamedata)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
