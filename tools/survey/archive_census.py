#!/usr/bin/env python3
"""Count members of the installed game's gamedata archives. Read-only.

Usage: archive_census.py [FA_DIR]   (default: $SUPERCOM_FA_DIR)
Prints counts only: no member content is read beyond zip metadata.
"""

import glob
import os
import sys
import zipfile

SUFFIXES = (".scm", ".sca", ".dds", ".bp", "_unit.bp", "_proj.bp", "_prop.bp")


def main() -> int:
    fa = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("SUPERCOM_FA_DIR", "")
    archives = sorted(glob.glob(os.path.join(fa, "gamedata", "*.scd")))
    if not archives:
        print(f"no archives under {fa!r}", file=sys.stderr)
        return 2
    entries = files = 0
    all_stored = True
    counts = dict.fromkeys(SUFFIXES, 0)
    for path in archives:
        with zipfile.ZipFile(path) as archive:
            for info in archive.infolist():
                entries += 1
                if info.is_dir():
                    continue
                files += 1
                all_stored &= info.compress_type == zipfile.ZIP_STORED
                name = info.filename.lower()
                for suffix in SUFFIXES:
                    if name.endswith(suffix):
                        counts[suffix] += 1
    print(f"archives {len(archives)} entries {entries} files {files} all_stored {all_stored}")
    print(" ".join(f"{suffix} {count}" for suffix, count in counts.items()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
