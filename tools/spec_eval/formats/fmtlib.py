"""Helpers shared by the formats and data-model evaluators. Standard library only.

Everything the sim helpers already provide (install resolution, SKIP, member
reading, the data-subset reader) is imported from simlib, with one exception:
`DupReader.table` is a copy of simlib's table rule with a key counter added, and
must change when that rule changes. This module adds only what the format and
data-model clauses need beyond simlib: archive holders per path, the duplicate-key
census over the blueprint reader, the sound-bank walk, and repository-source reads
for the clauses that describe code.
"""

import os
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), "sim"))
import simlib as S  # noqa: E402

Fail = S.Fail
expect = S.expect
show = S.show

# Constructors whose files the reader parses and the census counts (units, projectiles
# and props are the v0 inputs; beams and meshes parse but are not claimed as v0 inputs),
# and the two kinds the reader does not accept.
IN_SCOPE = ("UnitBlueprint", "ProjectileBlueprint", "PropBlueprint", "BeamBlueprint", "MeshBlueprint")
OUT_OF_SCOPE = ("EmitterBlueprint", "TrailEmitterBlueprint")


def archive_names(root: str) -> list[str]:
    """Archive stems under gamedata/, in ascending byte order of the file names."""
    folder = os.path.join(root, "gamedata")
    if not os.path.isdir(folder):
        raise Fail("missing directory gamedata")
    return sorted(f[:-4] for f in os.listdir(folder) if f.endswith(".scd"))


def holders(game: S.Game, root: str) -> dict[str, list[tuple[str, str]]]:
    """Lower-cased member path -> (archive, stored name) pairs, in mount order."""
    found: dict[str, list[tuple[str, str]]] = {}
    for stem in archive_names(root):
        for name in game.names(stem):
            found.setdefault(name.lower(), []).append((stem, name))
    return found


def constructor(text: str) -> str:
    """The identifier a blueprint file opens with, or '' when it opens with none."""
    word = ""
    for ch in text.lstrip():
        if ch.isalnum() or ch == "_":
            word += ch
        else:
            break
    return word


def blueprint_members(game: S.Game, root: str):
    """Yield (archive, member, constructor, text) for every .bp member of the install."""
    for stem in archive_names(root):
        for name in game.names(stem):
            if name.lower().endswith(".bp"):
                text = game.text(stem, name)
                yield stem, name, constructor(text), text


class DupReader(S._Parser):
    """The simlib reader, counting repeated named keys per table constructor.

    `table` is a copy of simlib's `_Parser.table` with the counter added (the
    one copied rule in this module); it must change when simlib's rule changes.
    """

    tables = 0
    repeats = 0

    def table(self):
        self.want("{")
        table, seen = S.Table(), set()
        while self.peek()[1] != "}":
            kind, text = self.peek()
            key = None
            if text == "[":
                self.take()
                key = self.value()
                self.want("]")
                self.want("=")
            elif kind == "name" and self.peek(1)[1] == "=":
                self.take()
                self.take()
                key = text
            if key is None:
                table.arr.append(self.value())
            else:
                if isinstance(key, str):
                    if key in seen:
                        DupReader.repeats += 1
                    seen.add(key)
                table[key] = self.value()
            if self.peek()[1] in (",", ";"):
                self.take()
            elif self.peek()[1] != "}":
                raise Fail(f"{self.where}: expected a separator")
        self.want("}")
        DupReader.tables += 1
        return table


def repo_text(relative: str) -> str:
    """A repository file as text; a missing file fails and names the path."""
    path = os.path.join(REPO, relative)
    try:
        with open(path, encoding="utf-8") as handle:
            return handle.read()
    except OSError:
        raise Fail(f"missing repository file {relative}") from None
