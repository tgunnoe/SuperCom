"""Shared helpers for the HUD specification evaluators (docs/specs/ui.md). Standard library only.

An evaluator imports this module after setting sys.dont_write_bytecode, reads the
installed game through SUPERCOM_FA_DIR only (never a Steam default), reads archive
members in memory, and writes nothing. Its only output is counts, constants,
identifiers, and one SKIP or FAIL line.

SKIP has one meaning: SUPERCOM_FA_DIR is unset or names no directory. With
SUPERCOM_REQUIRE_GAME=1 that case fails instead. A missing archive, member, or key
is always a failure that names the path.
"""

import os
import re
import sys
import zipfile
from collections import Counter

sys.dont_write_bytecode = True

FA_VAR = "SUPERCOM_FA_DIR"
REQUIRE_VAR = "SUPERCOM_REQUIRE_GAME"


class Fail(Exception):
    """The clause is false, or an input it needs is missing."""


def show(label: str, value: object) -> None:
    print(f"{label} {value}")


def expect(label: str, got: object, want: object) -> None:
    """Print a constant and fail unless it equals the pinned value."""
    show(label, got)
    if want is not None and got != want:
        raise Fail(f"{label} is {got}, the clause pins {want}")


def decode(raw: bytes) -> str:
    """Latin-1 text with CRLF folded to LF (the installed Lua is ISO-8859 with CRLF)."""
    return raw.decode("latin-1").replace("\r\n", "\n").replace("\r", "\n")


_COMMENT = re.compile(r"--\[(=*)\[.*?\]\1\]|--[^\n]*|#[^\n]*", re.S)
_CATEGORIES = re.compile(r"^[ \t]*Categories[ \t]*=[ \t]*\{(.*?)\}", re.S | re.M)
_BUILDABLE = re.compile(r"^[ \t]*BuildableCategory[ \t]*=[ \t]*\{(.*?)\}", re.S | re.M)
_ICON = re.compile(r"^[ \t]*StrategicIconName[ \t]*=[ \t]*['\"]([^'\"]*)['\"]", re.M)
_STRING = re.compile(r"'([^']*)'|\"([^\"]*)\"")


def strip_comments(text: str) -> str:
    return _COMMENT.sub("", text)


def strings(body: str) -> list[str]:
    return [a or b for a, b in _STRING.findall(body)]


class Game:
    """The installed game's archives, opened lazily and read in memory."""

    def __init__(self, root: str) -> None:
        self.root = root
        self._archives: dict[str, zipfile.ZipFile] = {}
        self._index: dict[str, str] | None = None
        self._units: dict[str, dict] | None = None

    def archive(self, name: str) -> zipfile.ZipFile:
        if name not in self._archives:
            path = os.path.join(self.root, "gamedata", f"{name}.scd")
            if not os.path.isfile(path):
                raise Fail(f"missing archive gamedata/{name}.scd")
            self._archives[name] = zipfile.ZipFile(path)
        return self._archives[name]

    def archives(self) -> list[str]:
        folder = os.path.join(self.root, "gamedata")
        return sorted(f[:-4] for f in os.listdir(folder) if f.endswith(".scd"))

    def members(self, archive: str, prefix: str = "") -> list[str]:
        """Member names of an archive, directory entries included, under a case-folded prefix."""
        return [n for n in self.archive(archive).namelist() if n.lower().startswith(prefix.lower())]

    def raw(self, archive: str, member: str) -> bytes:
        try:
            return self.archive(archive).read(member)
        except KeyError:
            raise Fail(f"missing member gamedata/{archive}.scd!{member}") from None

    def text(self, archive: str, member: str) -> str:
        return decode(self.raw(archive, member))

    def holders(self, prefix: str) -> Counter:
        """Archive name -> how many of its members lie under the case-folded prefix."""
        found: Counter = Counter()
        for name in self.archives():
            found[name] = len(self.members(name, prefix))
        return +found

    def index(self) -> dict[str, str]:
        """Lower-cased member path -> the archive that wins it (first mounted, byte order)."""
        if self._index is None:
            found: dict[str, str] = {}
            for name in self.archives():
                for member in self.archive(name).namelist():
                    if not member.endswith("/"):
                        found.setdefault(member.lower(), name)
            self._index = found
        return self._index

    def units(self) -> dict[str, dict]:
        """Unit blueprint ID -> {'cats': category strings, 'builds': BuildableCategory expressions, 'icon': StrategicIconName or None}."""
        if self._units is None:
            found = {}
            for name in self.members("units", "units/"):
                if name.lower().endswith("_unit.bp"):
                    ident = name.rsplit("/", 1)[-1][: -len("_unit.bp")].upper()
                    text = strip_comments(self.text("units", name))
                    cats = _CATEGORIES.search(text)
                    build = _BUILDABLE.search(text)
                    icon = _ICON.search(text)
                    found[ident] = {
                        "cats": set(strings(cats.group(1))) if cats else set(),
                        "builds": strings(build.group(1)) if build else [],
                        "icon": icon.group(1).lower() if icon else None,
                    }
            self._units = found
        return self._units


def selects(units: dict[str, dict], expr: str, ident: str) -> bool:
    """SIM-011's reading: an expression is the intersection of its identifiers, each a
    category string of the blueprint or its blueprint ID in lower case."""
    return all(t in units[ident]["cats"] or t == ident.lower() for t in expr.split())


def buildable_by(units: dict[str, dict], builder: str) -> set[str]:
    """The blueprints a builder's BuildableCategory selects under SIM-011's reading."""
    exprs = units[builder]["builds"]
    return {u for u in units if any(selects(units, e, u) for e in exprs)}


def guard_sim_reading(units: dict[str, dict]) -> None:
    """Fail unless this module's blueprint reading reproduces the totals SIM-010 and SIM-011 pin."""
    builders = [u for u, bp in units.items() if bp["builds"]]
    chosen = set().union(*(buildable_by(units, b) for b in builders))
    expect("reading_builders_with_BuildableCategory", len(builders), 105)
    expect("reading_expressions", sum(len(units[b]["builds"]) for b in builders), 215)
    expect("reading_blueprints_buildable_by_some_builder", len(chosen), 370)
    expect("reading_blueprints_buildable_by_no_builder", len(units) - len(chosen), 198)


def word_count(text: str, word: str) -> int:
    """How many times an identifier occurs in comment-free Lua text."""
    return len(re.findall(rf"\b{re.escape(word)}\b", strip_comments(text)))


def table_section(text: str, name: str, following: str | None) -> str:
    """The part of a data file from `name = {` up to `following = {` (or the end)."""
    start = re.search(rf"^{re.escape(name)}\s*=\s*\{{", text, re.M)
    if start is None:
        raise Fail(f"no table named {name}")
    stop = re.search(rf"^{re.escape(following)}\s*=\s*\{{", text[start.end() :], re.M) if following else None
    return text[start.end() : start.end() + stop.start()] if stop else text[start.end() :]


def key_actions(game: "Game", table: str, following: str | None) -> dict[str, str]:
    """Key action name -> its category, from lua/keymap/keyactions.lua."""
    text = strip_comments(game.text("lua", "lua/keymap/keyactions.lua"))
    section = table_section(text, table, following)
    heads = list(re.finditer(r"^\s*\['([^']+)'\]\s*=\s*\{", section, re.M))
    found: dict[str, str] = {}
    for at, head in enumerate(heads):
        body = section[head.end() : heads[at + 1].start() if at + 1 < len(heads) else len(section)]
        category = re.search(r"category\s*=\s*'([^']*)'", body)
        found[head.group(1)] = category.group(1) if category else ""
    return found


def key_bindings(game: "Game", table: str, following: str | None) -> dict[str, str]:
    """Key string -> the key action it is bound to, from lua/keymap/defaultKeyMap.lua."""
    text = strip_comments(game.text("lua", "lua/keymap/defaultKeyMap.lua"))
    section = table_section(text, table, following)
    return {k: v for k, v in re.findall(r"^\s*\['([^']+)'\]\s*=\s*'([^']*)'", section, re.M)}


def lua_defs(text: str) -> set[str]:
    """Names a Lua file defines as functions or classes (declared or assigned)."""
    pattern = re.compile(
        r"^\s*(?:local\s+)?function\s+([\w.:]+)\s*\(|^\s*(?:local\s+)?(\w+)\s*=\s*(?:function|Class\w*\s*[({])", re.M
    )
    return {a or b for a, b in pattern.findall(text)}


def install_dir() -> str | None:
    root = os.environ.get(FA_VAR, "")
    return root if root and os.path.isdir(root) else None


def run(checks: dict, argv: list[str]) -> int:
    """Run the check for the clause ID in argv; exit 2 on an unknown ID."""
    if len(argv) != 1 or argv[0] not in checks:
        known = " ".join(sorted(checks))
        print(f"usage: <script> <clause ID>; this script holds {known}", file=sys.stderr)
        return 2
    root = install_dir()
    if root is None:
        reason = f"{FA_VAR} is unset or names no directory"
        if os.environ.get(REQUIRE_VAR) == "1":
            print(f"FAIL: {reason}; {REQUIRE_VAR}=1 forbids continuing without the game")
            return 1
        print(f"SKIP: {reason}")
        return 0
    try:
        checks[argv[0]](Game(root))
    except Fail as error:
        print(f"FAIL: {error}")
        return 1
    except (zipfile.BadZipFile, OSError) as error:
        print(f"FAIL: cannot read the install: {error.__class__.__name__}")
        return 1
    return 0
