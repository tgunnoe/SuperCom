"""Shared helpers for the sim specification evaluators. Standard library only.

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


def words(items) -> str:
    return " ".join(sorted(items))


class Table(dict):
    """A Lua table: keyed entries in the dict, positional entries in `arr`."""

    def __init__(self) -> None:
        super().__init__()
        self.arr: list = []


_TOKEN = re.compile(
    r"""(?P<skip>\s+|--\[(?P<eq>=*)\[.*?\](?P=eq)\]|--[^\n]*|\#[^\n]*)
      |(?P<num>0[xX][0-9a-fA-F]+|(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)
      |(?P<str>'(?:[^'\\\n]|\\.)*'|"(?:[^"\\\n]|\\.)*")
      |(?P<name>[A-Za-z_]\w*)
      |(?P<sym>\.\.|[{}\[\](),;=.+\-*/%^:])""",
    re.S | re.X,
)


def _tokens(text: str, where: str) -> list[tuple[str, str]]:
    out, pos = [], 0
    while pos < len(text):
        found = _TOKEN.match(text, pos)
        if found is None:
            raise Fail(f"{where}: cannot tokenize at offset {pos}")
        pos = found.end()
        kind = found.lastgroup if found.lastgroup != "eq" else "skip"
        if kind != "skip":
            out.append((kind, found.group(kind)))
    return out


class _Parser:
    """The data subset of the Lua dialect that blueprints and scenario files use."""

    def __init__(self, text: str, where: str) -> None:
        self.where = where
        self.toks = _tokens(text, where)
        self.at = 0

    def peek(self, offset: int = 0) -> tuple[str, str]:
        index = self.at + offset
        return self.toks[index] if index < len(self.toks) else ("end", "")

    def take(self) -> tuple[str, str]:
        tok = self.peek()
        self.at += 1
        return tok

    def want(self, symbol: str) -> None:
        if self.take()[1] != symbol:
            raise Fail(f"{self.where}: expected {symbol!r} near token {self.at}")

    def value(self):
        left = self.term()
        while self.peek()[1] in ("+", "-") and isinstance(left, (int, float)):
            op = self.take()[1]
            right = self.term()
            left = left + right if op == "+" else left - right
        while self.peek()[1] == ".." and isinstance(left, str):
            self.take()
            left += str(self.term())
        return left

    def term(self):
        left = self.unary()
        while self.peek()[1] in ("*", "/") and isinstance(left, (int, float)):
            op = self.take()[1]
            right = self.unary()
            left = left * right if op == "*" else left / right
        return left

    def unary(self):
        kind, text = self.peek()
        if text == "-":
            self.take()
            return -self.unary()
        if text == "{":
            return self.table()
        self.take()
        if kind == "num":
            return int(text, 16) if text[:2] in ("0x", "0X") else float(text) if re.search(r"[.eE]", text) else int(text)
        if kind == "str":
            return text[1:-1]
        if kind == "name":
            if text in ("true", "false"):
                return text == "true"
            if text == "nil":
                return None
            while self.peek()[1] == ".":
                self.take()
                text += "." + self.take()[1]
            if self.peek()[1] == "{":  # a call with a table argument, such as Sound { ... }
                return self.table()
            if self.peek()[1] == "(":  # a call with arguments: the first argument stands for it
                self.take()
                first = self.value() if self.peek()[1] != ")" else None
                while self.peek()[1] == ",":
                    self.take()
                    self.value()
                self.want(")")
                return first
            return text
        raise Fail(f"{self.where}: unexpected token {text!r} near token {self.at}")

    def table(self) -> Table:
        self.want("{")
        table = Table()
        while self.peek()[1] != "}":
            kind, text = self.peek()
            if text == "[":
                self.take()
                key = self.value()
                self.want("]")
                self.want("=")
                table[key] = self.value()
            elif kind == "name" and self.peek(1)[1] == "=":
                self.take()
                self.take()
                table[text] = self.value()
            else:
                table.arr.append(self.value())
            if self.peek()[1] in (",", ";"):
                self.take()
            elif self.peek()[1] != "}":
                raise Fail(f"{self.where}: expected a separator near token {self.at}")
        self.want("}")
        return table


def parse_table(text: str, where: str) -> Table:
    """Parse the first table constructor of a blueprint-style file."""
    parser = _Parser(text, where)
    while parser.peek()[1] != "{":
        if parser.peek()[0] == "end":
            raise Fail(f"{where}: no table constructor")
        parser.take()
    return parser.table()


def parse_assignments(text: str, where: str) -> dict:
    """Parse a data file of `name = value` statements into a dict (locals included)."""
    parser = _Parser(text, where)
    out: dict = {}
    while parser.peek()[0] != "end":
        if parser.peek()[1] == "local":
            parser.take()
        kind, text_ = parser.peek()
        if kind == "name" and parser.peek(1)[1] == "=":
            parser.take()
            parser.take()
            out[text_] = parser.value()
        else:
            parser.take()
    return out


def decode(raw: bytes) -> str:
    """Latin-1 text with CRLF folded to LF (the installed Lua is ISO-8859 with CRLF)."""
    return raw.decode("latin-1").replace("\r\n", "\n").replace("\r", "\n")


class Game:
    """The installed game's archives, opened lazily and read in memory."""

    def __init__(self, root: str) -> None:
        self.root = root
        self._archives: dict[str, zipfile.ZipFile] = {}
        self._units: dict[str, Table] | None = None
        self._index: dict[str, str] | None = None

    def archive(self, name: str) -> zipfile.ZipFile:
        if name not in self._archives:
            path = os.path.join(self.root, "gamedata", f"{name}.scd")
            if not os.path.isfile(path):
                raise Fail(f"missing archive gamedata/{name}.scd")
            self._archives[name] = zipfile.ZipFile(path)
        return self._archives[name]

    def names(self, archive: str) -> list[str]:
        return [n for n in self.archive(archive).namelist() if not n.endswith("/")]

    def raw(self, archive: str, member: str) -> bytes:
        try:
            return self.archive(archive).read(member)
        except KeyError:
            raise Fail(f"missing member gamedata/{archive}.scd!{member}") from None

    def text(self, archive: str, member: str) -> str:
        return decode(self.raw(archive, member))

    def index(self) -> dict[str, str]:
        """Lower-cased member path -> the archive that wins it (first mounted, byte order)."""
        if self._index is None:
            folder = os.path.join(self.root, "gamedata")
            found: dict[str, str] = {}
            for file in sorted(f for f in os.listdir(folder) if f.endswith(".scd")):
                for name in self.names(file[:-4]):
                    found.setdefault(name.lower(), file[:-4])
            self._index = found
        return self._index

    def units(self) -> dict[str, Table]:
        """Every unit blueprint in units.scd, keyed by the identifier in its file name."""
        if self._units is None:
            found = {}
            for name in self.names("units"):
                if name.lower().endswith("_unit.bp"):
                    ident = name.rsplit("/", 1)[-1][: -len("_unit.bp")].upper()
                    found[ident] = parse_table(self.text("units", name), name)
            self._units = found
        return self._units


def categories(bp: Table) -> set[str]:
    return {c for c in bp.get("Categories", Table()).arr if isinstance(c, str)}


def block(bp: Table, *path: str) -> Table:
    """A nested table of a blueprint, or an empty one when the path is absent."""
    node = bp
    for key in path:
        node = node.get(key)
        if not isinstance(node, Table):
            return Table()
    return node


def key_census(tables) -> Counter:
    census: Counter = Counter()
    for table in tables:
        census.update(k for k in table if isinstance(k, str))
    return census


def install_dir() -> str | None:
    root = os.environ.get(FA_VAR, "")
    return root if root and os.path.isdir(root) else None


def run(checks: dict, argv: list[str], offline: frozenset = frozenset()) -> int:
    """Run the check for the clause ID in argv; exit 2 on an unknown ID.

    An ID listed in `offline` reads only the repository, so it runs without an install.
    """
    if len(argv) != 1 or argv[0] not in checks:
        known = " ".join(sorted(checks))
        print(f"usage: <script> <clause ID>; this script holds {known}", file=sys.stderr)
        return 2
    root = install_dir()
    if argv[0] in offline:
        try:
            checks[argv[0]](None)
        except Fail as error:
            print(f"FAIL: {error}")
            return 1
        return 0
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


_DEF = re.compile(
    r"^\s*(?:local\s+)?function\s+([\w.:]+)\s*\(|^\s*(?:local\s+)?(\w+)\s*=\s*"
    r"(?:function|Class\w*\s*[({]|State\s*[({])",
    re.M,
)


def lua_defs(text: str) -> set[str]:
    """Names a Lua file defines as functions, classes, or states (declared or assigned)."""
    return {a or b for a, b in _DEF.findall(text)}


def need_defs(game: "Game", archive: str, member: str, names, label: str) -> None:
    """Fail unless the member defines every name; print how many it was asked for."""
    winner = game.index().get(member.lower())
    if winner != archive:
        raise Fail(f"{member} is won by {winner}, not {archive}")
    have = lua_defs(game.text(archive, member))
    missing = sorted(set(names) - have)
    show(label, len(set(names)))
    if missing:
        raise Fail(f"gamedata/{archive}.scd!{member} does not define {' '.join(missing)}")


def member_hits(game: "Game", archive: str, needles) -> dict[str, list[str]]:
    """For each needle, the Lua members of an archive whose text contains it."""
    hits: dict[str, list[str]] = {n: [] for n in needles}
    for name in game.names(archive):
        if name.lower().endswith(".lua"):
            text = game.text(archive, name)
            for needle in needles:
                if needle in text:
                    hits[needle].append(name)
    return hits
