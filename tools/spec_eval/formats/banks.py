#!/usr/bin/env python3
"""Evaluator for the sound-bank inventory clause of docs/specs/formats.md (FMT-057).

Counts wave banks and sound banks anywhere under sounds/ and reads the four-byte magic
and the version word of each wave bank. It decodes nothing: the codec inventory is open.
"""

import os
import struct
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fmtlib as F


def inventory(_game) -> None:
    root = os.path.join(os.environ[F.S.FA_VAR], "sounds")
    if not os.path.isdir(root):
        raise F.Fail("missing directory sounds")
    wave, sound, heads = 0, 0, Counter()
    for folder, _, names in os.walk(root):
        for name in names:
            low = name.lower()
            if low.endswith(".xsb"):
                sound += 1
            elif low.endswith(".xwb"):
                wave += 1
                with open(os.path.join(folder, name), "rb") as handle:
                    head = handle.read(8)
                if len(head) < 8:
                    raise F.Fail(f"wave bank {name} is shorter than 8 bytes")
                heads[(head[:4].decode("latin-1"), struct.unpack("<I", head[4:8])[0])] += 1
    F.expect("top_level_entries", len(os.listdir(root)), 160)
    F.expect("wave_banks", wave, 89)
    F.expect("sound_banks", sound, 90)
    F.expect("wave_bank_heads", dict(heads), {("WBND", 43): 89})


CHECKS = {"FMT-057": inventory}

if __name__ == "__main__":
    sys.exit(F.S.run(CHECKS, sys.argv[1:]))
