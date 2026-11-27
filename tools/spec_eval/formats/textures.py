#!/usr/bin/env python3
"""Evaluator for the texture and map-mask clauses of docs/specs/formats.md (FMT-075, FMT-077 to FMT-079).

A second decoder, written from the clauses and sharing no code with the Rust crate: it decodes the
installed textures in place, one file at a time, holding none, and prints counts and digests only.
Standard library only.

    textures.py FMT-077   the ground textures of the 60 maps (and the colour-block census over them)
    textures.py FMT-078   the two masks of every map
    textures.py FMT-075   the map census
    textures.py units     the units' two textures (about 3 GiB decoded; run at reconcile and at close,
                          not by the specification lint, whose clause for the set names a Rust test)
    textures.py --measure prints every number the clauses state

Digest (FMT-077): the digest of a file is the CRC-32 (the zlib polynomial, as `zlib.crc32`) of its decoded
levels, top level first, each level's bytes in the order red, green, blue, alpha, run on as one stream; the
combined digest of a set is FNV-1a over 64 bits (offset basis 0xcbf29ce484222325, prime 0x100000001b3) of
the four little-endian bytes of each file's digest, files in ascending order of normalized path.
"""

import multiprocessing
import os
import re
import struct
import sys
import time
import zlib

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fmtlib as F

MAX_SIDE = 8192
FNV_BASIS = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3
MASK64 = (1 << 64) - 1


class Refused(Exception):
    """A texture the clauses refuse; the message names what was found."""


def le32(b, at):
    return struct.unpack_from("<I", b, at)[0]


# --- the decoder, from FMT-061 to FMT-072 ---------------------------------------------------------


def expand565(c):
    r, g, b = c >> 11, (c >> 5) & 63, c & 31
    return ((r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2))


def nearest(n, d):
    """n / d rounded to the nearest integer, a tie rounding up (FMT-065)."""
    return (n + d // 2) // d


def colour_table(c0, c1, three_colour_allowed):
    e0, e1 = expand565(c0), expand565(c1)
    if c0 > c1 or not three_colour_allowed:
        two = tuple(nearest(2 * a + b, 3) for a, b in zip(e0, e1))
        three = tuple(nearest(a + 2 * b, 3) for a, b in zip(e0, e1))
        return [e0 + (255,), e1 + (255,), two + (255,), three + (255,)]
    mid = tuple(nearest(a + b, 2) for a, b in zip(e0, e1))
    return [e0 + (255,), e1 + (255,), mid + (255,), (0, 0, 0, 0)]


def alpha_table(a0, a1):
    if a0 > a1:
        return [a0, a1] + [nearest((7 - k) * a0 + k * a1, 7) for k in range(1, 7)]
    return [a0, a1] + [nearest((5 - k) * a0 + k * a1, 5) for k in range(1, 5)] + [0, 255]


def level_dims(w, h, i):
    return max(1, w >> i), max(1, h >> i)


def block_len(fmt):
    return 8 if fmt == "DXT1" else 16


def chain_blocks_bytes(fmt, w, h, mips):
    total = 0
    for i in range(mips):
        lw, lh = level_dims(w, h, i)
        total += max(1, -(-lw // 4)) * max(1, -(-lh // 4)) * block_len(fmt)
    return total


def decode_block_level(fmt, data, off, lw, lh, stats):
    bw, bh = max(1, -(-lw // 4)), max(1, -(-lh // 4))
    size = block_len(fmt)
    out = bytearray(lw * lh * 4)
    for by in range(bh):
        rows = min(4, lh - by * 4)
        for bx in range(bw):
            blk = data[off : off + size]
            off += size
            colour = blk[size - 8 :]
            c0, c1, codes = struct.unpack("<HHI", colour)
            pal = [bytes(p) for p in colour_table(c0, c1, fmt == "DXT1")]
            if fmt != "DXT1":
                if c0 <= c1 and any(((codes >> (2 * i)) & 3) >= 2 for i in range(16)):
                    stats["late_blocks"] += 1
                if fmt == "DXT3":
                    word = int.from_bytes(blk[:8], "little")
                    alphas = [17 * ((word >> (4 * i)) & 15) for i in range(16)]
                else:
                    table = alpha_table(blk[0], blk[1])
                    word = int.from_bytes(blk[2:8], "little")
                    alphas = [table[(word >> (3 * i)) & 7] for i in range(16)]
            cols = min(4, lw - bx * 4)
            for y in range(rows):
                row = bytearray()
                for x in range(cols):
                    p = pal[(codes >> (2 * (4 * y + x))) & 3]
                    if fmt == "DXT1":
                        row += p
                    else:
                        row += p[:3] + bytes((alphas[4 * y + x],))
                at = ((by * 4 + y) * lw + bx * 4) * 4
                out[at : at + len(row)] = row
    return out


def mask_shift(mask, what):
    for k in range(4):
        if mask == 0xFF << (8 * k):
            return k
    raise Refused(f"{what} mask {mask:#010x} is not one byte-aligned run of eight bits")


def decode_dds(raw, stats=None):
    """Decode every level of a texture; returns [(w, h, rgba bytes)], or raises Refused."""
    stats = stats if stats is not None else {"late_blocks": 0}
    if len(raw) < 128:
        raise Refused(f"truncated header: {len(raw)} bytes")
    if raw[:4] != b"DDS ":
        raise Refused("bad magic")
    if le32(raw, 4) != 124:
        raise Refused("dwSize is not 124")
    flags, h, w = le32(raw, 8), le32(raw, 12), le32(raw, 16)
    mips = le32(raw, 28) if flags & 0x20000 else 1
    caps2 = le32(raw, 112)
    if caps2 & 0x200 or caps2 & 0x200000:
        raise Refused("cube map or volume texture")
    if w == 0 or h == 0 or w > MAX_SIDE or h > MAX_SIDE:
        raise Refused(f"dimensions {w}x{h}")
    longest = max(w, h)
    if mips < 1 or mips > longest.bit_length():
        raise Refused(f"mip count {mips} for {w}x{h}")
    pf = le32(raw, 80)
    data = raw[128:]
    levels = []
    if pf & 4:
        fmt = raw[84:88].decode("latin-1")
        if fmt not in ("DXT1", "DXT3", "DXT5"):
            raise Refused(f"fourcc {fmt!r}")
        need = chain_blocks_bytes(fmt, w, h, mips)
        if len(data) < need:
            raise Refused(f"data {len(data)} bytes, chain {need}")
        off = 0
        for i in range(mips):
            lw, lh = level_dims(w, h, i)
            levels.append((lw, lh, decode_block_level(fmt, data, off, lw, lh, stats)))
            off += max(1, -(-lw // 4)) * max(1, -(-lh // 4)) * block_len(fmt)
        return levels
    if le32(raw, 88) != 32:
        raise Refused(f"uncompressed texture of {le32(raw, 88)} bits")
    masks = [le32(raw, 92), le32(raw, 96), le32(raw, 100), le32(raw, 104)]
    shifts = [mask_shift(m, n) for m, n in zip(masks[:3], "RGB")]
    if masks[3]:
        shifts.append(mask_shift(masks[3], "A"))
    if len(set(shifts)) != len(shifts):
        raise Refused("overlapping channel masks")
    need = sum(lw * lh * 4 for lw, lh in (level_dims(w, h, i) for i in range(mips)))
    if len(data) < need:
        raise Refused(f"data {len(data)} bytes, chain {need}")
    off = 0
    for i in range(mips):
        lw, lh = level_dims(w, h, i)
        n = lw * lh
        src = data[off : off + 4 * n]
        out = bytearray(4 * n)
        for c in range(3):
            out[c::4] = src[shifts[c] :: 4]
        if masks[3]:
            out[3::4] = src[shifts[3] :: 4]
        else:
            out[3::4] = b"\xff" * n
        levels.append((lw, lh, out))
        off += 4 * n
    return levels


def file_digest(levels):
    crc = 0
    for _, _, pixels in levels:
        crc = zlib.crc32(pixels, crc)
    return crc


def combine(crcs):
    state = FNV_BASIS
    for crc in crcs:
        for byte in struct.pack("<I", crc):
            state = ((state ^ byte) * FNV_PRIME) & MASK64
    return state


# --- the map, from FMT-074 --------------------------------------------------------------------------


class Cursor:
    def __init__(self, b):
        self.b, self.p = b, 0

    def take(self, n):
        if n < 0 or self.p + n > len(self.b):
            raise Refused("map truncated")
        s = self.b[self.p : self.p + n]
        self.p += n
        return s

    def i32(self):
        return struct.unpack("<i", self.take(4))[0]

    def cstr(self):
        end = self.b.index(b"\0", self.p)
        s = self.b[self.p : end]
        self.p = end + 1
        return s.decode("latin-1")


def read_map(b):
    r = Cursor(b)
    r.take(30)
    r.take(r.i32())
    version = r.i32()
    w, h = r.i32(), r.i32()
    r.take(4 + 2 * (w + 1) * (h + 1) + 1)
    program = r.cstr()
    r.cstr()
    r.cstr()
    for _ in range(r.i32()):
        r.cstr()
        r.cstr()
    r.take(23 * 4 + 1 + 23 * 4)
    r.cstr()
    r.cstr()
    r.take(16)
    for _ in range(4):
        r.take(8)
        r.cstr()
    for _ in range(r.i32()):
        r.cstr()
        r.cstr()
        r.take(17 * 4)
    r.take(24 + (4 if version > 56 else 0))
    strata = []
    for _ in range(10):
        strata.append(r.cstr())
        r.take(4)
    for _ in range(9):
        r.cstr()
        r.take(4)
    r.take(8)
    for _ in range(r.i32()):
        r.take(8)
        for _ in range(r.i32()):
            r.take(r.i32())
        r.take(48)
    for _ in range(r.i32()):
        r.take(4)
        r.cstr()
        r.take(4 * r.i32())
    r.take(8)
    normals = r.i32()
    for _ in range(normals):
        r.take(r.i32())
    low = bytes(r.take(r.i32()))
    high = bytes(r.take(r.i32()))
    return {"cells": (w, h), "program": program, "strata": strata, "normals": normals, "low": low, "high": high}


def map_files(root):
    folder = os.path.join(root, "maps")
    found = []
    for d in sorted(os.listdir(folder)):
        full = os.path.join(folder, d)
        if os.path.isdir(full):
            found += [os.path.join(full, f) for f in sorted(os.listdir(full)) if f.lower().endswith(".scmap")]
    return found


def normalize(path):
    return (path[1:] if path.startswith("/") else path).replace("\\", "/").lower()


# --- the sets ---------------------------------------------------------------------------------------


_GAMES = {}


def _game(root):
    if root not in _GAMES:
        _GAMES[root] = F.S.Game(root)
    return _GAMES[root]


def _work(job):
    root, kind, key = job
    stats = {"late_blocks": 0}
    if kind == "member":
        archive, member = key
        raw = _game(root).raw(archive, member)
    else:
        raw = key
    levels = decode_dds(raw, stats)
    return file_digest(levels), len(levels), sum(len(p) for _, _, p in levels), stats["late_blocks"]


def sweep(root, jobs, label):
    started = time.time()
    with multiprocessing.Pool() as pool:
        results = pool.map(_work, jobs, chunksize=1)
    crcs = [r[0] for r in results]
    total = {
        "files": len(results),
        "levels": sum(r[1] for r in results),
        "bytes": sum(r[2] for r in results),
        "late_colour_blocks": sum(r[3] for r in results),
        "digest": combine(crcs),
    }
    F.show(f"{label} sweep seconds", round(time.time() - started, 1))
    return total, crcs


def all_maps(root):
    return [read_map(open(p, "rb").read()) for p in map_files(root)]


def member_name(game, archive, lower):
    for name in game.names(archive):
        if name.lower() == lower:
            return name
    raise F.Fail(f"missing {lower}")


def ground_jobs(game, root, maps):
    index = game.index()
    names = sorted({normalize(t) for m in maps for t in m["strata"] if t})
    by_archive = {}
    jobs = []
    for n in names:
        a = index[n]
        if a not in by_archive:
            by_archive[a] = {x.lower(): x for x in game.names(a)}
        jobs.append((root, "member", (a, by_archive[a][n])))
    return jobs, names


def units_jobs(game, root):
    index = game.index()
    by = {x.lower(): x for x in game.names("units")}
    ids = sorted(
        n[len("units/") : -len("_unit.bp")].split("/")[-1]
        for n in index
        if n.startswith("units/") and n.endswith("_unit.bp") and index[n] == "units"
    )
    keys = []
    for i in ids:
        a, s = f"units/{i}/{i}_albedo.dds", f"units/{i}/{i}_specteam.dds"
        if a in index and s in index:
            keys += [a, s]
    keys.sort()
    return [(root, "member", (index[k], by[k] if k in by else _find(game, index[k], k))) for k in keys], keys


def _find(game, archive, lower):
    return member_name(game, archive, lower)


# --- the clauses ------------------------------------------------------------------------------------

GROUND = {"files": 114, "late_colour_blocks": None, "digest": None}


def measure_ground(game):
    root = os.environ[F.S.FA_VAR]
    maps = all_maps(root)
    jobs, names = ground_jobs(game, root, maps)
    return sweep(root, jobs, "ground")[0], len(names)


def measure_masks(game):
    root = os.environ[F.S.FA_VAR]
    maps = all_maps(root)
    jobs = []
    for m in maps:
        jobs += [(root, "blob", m["low"]), (root, "blob", m["high"])]
    return sweep(root, jobs, "masks")[0], maps


def measure_units(game):
    root = os.environ[F.S.FA_VAR]
    jobs, keys = units_jobs(game, root)
    return sweep(root, jobs, "units")[0]


def map_census(maps, root):
    census = {"maps": len(maps), "normal_maps": {}, "none_of_5_to_8": 0, "equal_masks": 0, "programs": {}}
    sizes = {}
    ok_masks = True
    for m in maps:
        census["normal_maps"][m["normals"]] = census["normal_maps"].get(m["normals"], 0) + 1
        census["none_of_5_to_8"] += all(m["strata"][i] == "" for i in range(5, 9))
        census["equal_masks"] += m["low"] == m["high"]
        census["programs"][m["program"]] = census["programs"].get(m["program"], 0) + 1
        for blob in (m["low"], m["high"]):
            lv = decode_dds(blob)
            hdr_bits = le32(blob, 88)
            ok = (
                len(lv) == 1
                and hdr_bits == 32
                and not (le32(blob, 80) & 4)
                and lv[0][:2] == (m["cells"][0] // 2, m["cells"][1] // 2)
                and len(blob) == 128 + lv[0][0] * lv[0][1] * 4
            )
            ok_masks &= ok
            sizes[lv[0][:2]] = sizes.get(lv[0][:2], 0) + 1
    census["masks_whole_half_size_uncompressed_32"] = ok_masks
    census["mask_sizes"] = dict(sorted(sizes.items()))
    census["programs"] = dict(sorted(census["programs"].items()))
    census["normal_maps"] = dict(sorted(census["normal_maps"].items()))
    return census


def measure(game):
    root = os.environ[F.S.FA_VAR]
    maps = all_maps(root)
    F.show("map census", map_census(maps, root))
    ground, names = measure_ground(game)
    F.show("ground", ground)
    masks, _ = measure_masks(game)
    F.show("masks", masks)


EXPECTED = {
    "ground": {
        "files": 114,
        "levels": 1020,
        "bytes": 160772940,
        "late_colour_blocks": 0,
        "digest": 5853751105962280521,
    },
    "masks": {
        "files": 120,
        "levels": 120,
        "bytes": 212074496,
        "late_colour_blocks": 0,
        "digest": 9191786234201547060,
    },
    "units": {
        "files": 1052,
        "levels": 10473,
        "bytes": 3079054692,
        "late_colour_blocks": 0,
        "digest": 9925447467578818197,
    },
    "census": {
        "maps": 60,
        "normal_maps": {1: 57, 4: 3},
        "none_of_5_to_8": 41,
        "equal_masks": 40,
        "programs": {"TTerrain": 39, "TTerrainGlow": 1, "TTerrainXP": 20},
        "masks_whole_half_size_uncompressed_32": True,
        "mask_sizes": {(128, 128): 20, (256, 256): 36, (512, 512): 44, (1024, 512): 2, (1024, 1024): 12, (2048, 2048): 6},
    },
}


def expect_set(label, got, key):
    want = EXPECTED[key]
    for field, value in want.items():
        F.expect(f"{label} {field}", got[field], value)


def check_ground(game):
    got, _ = measure_ground(game)
    expect_set("ground textures", got, "ground")


def check_masks(game):
    got, _ = measure_masks(game)
    expect_set("masks", got, "masks")


def check_census(game):
    root = os.environ[F.S.FA_VAR]
    got = map_census(all_maps(root), root)
    for field, value in EXPECTED["census"].items():
        F.expect(f"map census {field}", got[field], value)


def check_units(game):
    expect_set("units textures", measure_units(game), "units")


def check_measure(game):
    measure(game)
    F.show("units", measure_units(game))


CHECKS = {
    "FMT-075": check_census,
    "FMT-077": check_ground,
    "FMT-078": check_masks,
    "units": check_units,
    "measure": check_measure,
    "measure-quick": measure,
}

if __name__ == "__main__":
    sys.exit(F.S.run(CHECKS, sys.argv[1:]))
