#!/usr/bin/env python3
"""Tabulate SCM, SCA, and SCMAP header versions in the installed game. Read-only.

Usage: format_versions.py [FA_DIR]   (default: $SUPERCOM_FA_DIR)
Reads only fixed-size headers; prints counts and the names of version
outliers (archive member paths are identifiers, not content).
"""

import collections
import glob
import os
import struct
import sys
import zipfile

SCMAP_SIGNATURE = 0x1A70614D


def scmap_versions(path: str) -> tuple[int, int, int]:
    with open(path, "rb") as handle:
        signature, major, _, _ = struct.unpack("<iiii", handle.read(16))
        handle.read(8 + 4 + 2)  # width, height, int 0, short 0
        (preview_len,) = struct.unpack("<i", handle.read(4))
        handle.seek(preview_len, os.SEEK_CUR)
        (inner,) = struct.unpack("<i", handle.read(4))
    return signature, major, inner


def main() -> int:
    fa = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("SUPERCOM_FA_DIR", "")
    scm = collections.Counter()
    sca = collections.Counter()
    outliers = []
    for path in sorted(glob.glob(os.path.join(fa, "gamedata", "*.scd"))):
        with zipfile.ZipFile(path) as archive:
            for info in archive.infolist():
                name = info.filename.lower()
                if not (name.endswith(".scm") or name.endswith(".sca")):
                    continue
                with archive.open(info) as member:
                    magic, version = struct.unpack("<4sI", member.read(8))
                counter = scm if name.endswith(".scm") else sca
                counter[(magic.decode("ascii", "replace"), version)] += 1
                if version != 5:
                    outliers.append(f"{info.filename} v{version} {info.file_size}B")
    maps = collections.Counter()
    for path in sorted(glob.glob(os.path.join(fa, "maps", "*", "*.scmap"))):
        signature, major, inner = scmap_versions(path)
        maps[(signature == SCMAP_SIGNATURE, major, inner)] += 1
    print("scm", dict(scm))
    print("sca", dict(sca))
    print("scmap (signature_ok, major, inner)", dict(maps))
    print("outliers", outliers)
    return 0


if __name__ == "__main__":
    sys.exit(main())
