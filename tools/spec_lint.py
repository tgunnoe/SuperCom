#!/usr/bin/env python3
"""Lint the SuperCom specification in docs/specs. Standard library only.

Usage (exactly one mode per run):
  spec_lint.py --check PATH         clause grammar and evaluator references in PATH
  spec_lint.py --inventory FILE     engine-API inventory table (--min-engine-rows N, default 127)
  spec_lint.py --coverage SPEC DIR  clause minimums per domain; SPEC is DOMAIN=MIN/VERIFIED,...
  spec_lint.py --require ID FILE    succeed only if FILE holds a well-formed clause with ID
  spec_lint.py --self-test          run every fixture under tools/spec_lint/fixtures/

Exit status: 0 clean, 1 findings (printed as file:line: message), 2 usage error.

The clause form is documented in docs/specs/README.md. In short, a clause is a
paragraph that starts with the header line

  **<TAG>-<NNN>** `<evidence class>` `<evaluator reference>`

followed by the statement text up to the first blank line. A line that starts with
`**` is always a clause header; fenced code and HTML comments are not scanned.

An inventory is a pipe table whose header names Name, Owner, and Citation (any order,
other columns allowed). Every row needs a name, an owner of engine or lua, and a
citation. Other tables in the file are ignored.

Fixtures are tools/spec_lint/fixtures/NAME.md with a sibling NAME.expect. The first
line of the expectation is the expected exit code; every later line is a substring
the output must contain. A name starting with "green-" must expect 0, any other name
a non-zero code. A fixture runs `--check` on itself unless its first line is a
directive `<!-- spec-lint: ARGS -->`; ARGS then replace the default arguments, with
{file} standing for the fixture and {dir} for the fixtures directory.
"""

import argparse
import contextlib
import functools
import io
import os
import re
import shlex
import subprocess
import sys
from collections.abc import Iterator, Sequence
from dataclasses import dataclass, field
from pathlib import Path

TOOL_DIR = Path(__file__).resolve().parent
REPO_ROOT = TOOL_DIR.parent
FIXTURES = TOOL_DIR / "spec_lint" / "fixtures"

EVIDENCE_CLASSES = ("game-data", "reference", "engine-stub", "design", "derived")
CLASS_LIST = ", ".join(sorted(EVIDENCE_CLASSES))
EVALUATOR_KINDS = ("test", "script", "source")
VERIFIED_KINDS = frozenset({"test", "script"})
MISSING_CLASS = f"missing evidence class (one of {CLASS_LIST})"
MISSING_EVALUATOR = "missing evaluator reference (test:, script:, or source:)"

# A clause ID never matches this pattern, so it cannot be mistaken for a node id.
RESERVED_ID = re.compile(r"\b[NPFC][1-9][0-9]*\b")
CLAUSE_ID = re.compile(r"[A-Za-z]{3,}-[0-9]{3}")
LOOSE_ID = re.compile(r"(?P<tag>[A-Za-z]+)-(?P<number>[0-9]+)")
HEADER = re.compile(r"\*\*(?P<ident>[^*`]*)\*\*(?P<spans>(?: `[^`]*`)*)(?P<tail>.*)")
SPAN = re.compile(r"`([^`]*)`")
FENCE = re.compile(r" {0,3}(?P<run>`{3,}|~{3,})")
DIRECTIVE = re.compile(r"<!-- spec-lint: (?P<args>.*) -->")
QUOTA = re.compile(r"(?P<domain>[a-z0-9][a-z0-9-]*)=(?P<clauses>[0-9]+)/(?P<verified>[0-9]+)")

NEXTEST_LIST = ("cargo", "nextest", "list", "--workspace", "--color", "never")
NEXTEST_TIMEOUT_SECONDS = 900

DEFAULT_MIN_ENGINE_ROWS = 127
INVENTORY_COLUMNS = ("name", "owner", "citation")
OWNERS = ("engine", "lua")
DELIMITER_CELL = re.compile(r":?-+:?")
UNESCAPED_PIPE = re.compile(r"(?<!\\)\|")

MIN_RED_FIXTURES = 8
MIN_GREEN_FIXTURES = 2


@dataclass(frozen=True, slots=True)
class Finding:
    path: str
    line: int
    message: str

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.message}"


@dataclass(frozen=True, slots=True)
class Header:
    """A parsed clause header line. `problems` holds its syntax findings."""

    ident: str
    evidence: str | None
    refs: tuple[str, ...]
    ident_ok: bool
    problems: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class Clause:
    path: str
    line: int
    header: Header
    statement: tuple[str, ...]
    starts_paragraph: bool

    @property
    def problems(self) -> list[str]:
        """Why the clause is not well-formed; empty when it is."""
        problems = list(self.header.problems)
        if not self.statement:
            problems.append("clause has no statement text")
        if not self.starts_paragraph:
            problems.append("clause header must start a paragraph (put a blank line before it)")
        return problems

    @property
    def kind(self) -> str | None:
        """The evaluator kind: test, script, or source."""
        return ref_kind(self.header.refs[0]) if len(self.header.refs) == 1 else None


@dataclass(slots=True)
class Draft:
    line: int
    header: str
    starts_paragraph: bool
    statement: list[str] = field(default_factory=list)


@dataclass(frozen=True, slots=True)
class Tests:
    """The workspace test listing, or why it could not be produced."""

    lines: tuple[str, ...]
    error: str | None


@dataclass(frozen=True, slots=True)
class Quota:
    """The minimums one domain file must meet."""

    domain: str
    clauses: int
    verified: int


@dataclass(frozen=True, slots=True)
class InventoryRow:
    line: int
    owner: str
    problems: tuple[str, ...]


class UsageError(Exception):
    """The command line is wrong; reported through argparse as exit status 2."""


def count(number: int, noun: str) -> str:
    return f"{number} {noun}" if number == 1 else f"{number} {noun}s"


def live_lines(text: str) -> Iterator[tuple[int, str]]:
    """Yield (number, line) for every line; fenced code and HTML comments read as blank."""
    fence = ""
    in_comment = False
    for number, line in enumerate(text.splitlines(), start=1):
        if fence:
            body = line.strip()
            if len(body) >= len(fence) and body == fence[0] * len(body):
                fence = ""
            yield number, ""
        elif in_comment:
            in_comment = "-->" not in line
            yield number, ""
        elif opening := FENCE.match(line):
            fence = opening["run"]
            yield number, ""
        elif line.lstrip().startswith("<!--"):
            in_comment = "-->" not in line
            yield number, ""
        else:
            yield number, line


def id_problems(ident: str) -> list[str]:
    if RESERVED_ID.search(ident):
        return [f"clause ID {ident!r} matches the reserved node-id pattern {RESERVED_ID.pattern}"]
    if CLAUSE_ID.fullmatch(ident):
        return []
    loose = LOOSE_ID.fullmatch(ident)
    if loose and len(loose["tag"]) < 3:
        return [f"clause ID {ident!r}: tag {loose['tag']!r} must be three or more ASCII letters"]
    if loose and len(loose["number"]) != 3:
        return [f"clause ID {ident!r}: the number must be exactly three digits"]
    return [f"clause ID {ident!r} is not of the form <TAG>-<NNN>"]


def ref_kind(ref: str) -> str | None:
    kind, colon, _ = ref.partition(":")
    return kind if colon and kind in EVALUATOR_KINDS else None


def parse_header(text: str) -> Header:
    found = HEADER.fullmatch(text.rstrip())
    if found is None:
        problem = "malformed clause header: expected **ID** `class` `evaluator`"
        return Header("", None, (), False, (problem,))
    ident = found["ident"]
    spans = SPAN.findall(found["spans"])
    problems = id_problems(ident)
    ident_ok = not problems
    tail = found["tail"].strip()
    if tail:
        problems.append(f"unexpected text {tail!r} in the header; write **ID** `class` `evaluator`")
    if not spans:
        evidence, refs = None, []
    elif len(spans) == 1 and ":" in spans[0]:
        evidence, refs = None, spans
    else:
        evidence, refs = spans[0], spans[1:]
    if evidence is None:
        problems.append(MISSING_CLASS)
    elif evidence not in EVIDENCE_CLASSES:
        problems.append(f"unknown evidence class {evidence!r} (one of {CLASS_LIST})")
    if not refs:
        problems.append(MISSING_EVALUATOR)
    elif len(refs) > 1:
        problems.append(f"found {len(refs)} evaluator references; a clause has exactly one")
    problems += [
        f"evaluator reference {ref!r} must begin with test:, script:, or source:"
        for ref in refs
        if ref_kind(ref) is None
    ]
    return Header(ident, evidence, tuple(refs), ident_ok, tuple(problems))


def scan(path: str, text: str) -> list[Clause]:
    """Every clause in a document, well-formed or not."""
    drafts: list[Draft] = []
    current: Draft | None = None
    after_blank = True
    for number, line in live_lines(text):
        if line.startswith("**"):
            current = Draft(number, line, after_blank)
            drafts.append(current)
        elif not line.strip():
            current = None
        elif current is not None:
            current.statement.append(line.strip())
        after_blank = not line.strip()
    return [
        Clause(path, d.line, parse_header(d.header), tuple(d.statement), d.starts_paragraph)
        for d in drafts
    ]


@functools.cache
def workspace_tests() -> Tests:
    """`cargo nextest list --workspace`, run once per process."""
    try:
        done = subprocess.run(
            NEXTEST_LIST,
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=NEXTEST_TIMEOUT_SECONDS,
            check=False,
        )
    except FileNotFoundError:
        return Tests((), "`cargo` is not on PATH")
    except subprocess.TimeoutExpired:
        return Tests((), f"`cargo nextest list` timed out after {NEXTEST_TIMEOUT_SECONDS}s")
    if done.returncode != 0:
        last = (done.stderr.strip().splitlines() or [""])[-1]
        return Tests((), f"`cargo nextest list` exited {done.returncode}: {last}")
    return Tests(tuple(line.strip() for line in done.stdout.splitlines() if line.strip()), None)


def names_test(name: str, listing_line: str) -> bool:
    """True if name occurs in a listing line as whole path segments (a segment or suffix)."""
    before = r"(?:^|(?<=\s)|(?<=/)|(?<=::))"
    after = r"(?:$|(?=\s)|(?=/)|(?=::))"
    return re.search(before + re.escape(name) + after, listing_line) is not None


def unresolved_test(name: str) -> str | None:
    if not name:
        return "test reference is empty"
    listing = workspace_tests()
    if listing.error:
        return f"test reference {name!r} cannot be checked: {listing.error}"
    if not any(names_test(name, line) for line in listing.lines):
        return f"test reference {name!r} matches no test in `cargo nextest list --workspace`"
    return None


def unresolved_script(payload: str) -> str | None:
    words = payload.split(maxsplit=1)
    if not words:
        return "script reference is empty"
    path = REPO_ROOT / words[0]
    if not path.exists():
        return f"script {words[0]!r} does not exist"
    if not path.is_file():
        return f"script {words[0]!r} is not a file"
    if path.suffix != ".py" and not os.access(path, os.X_OK):
        return f"script {words[0]!r} is neither executable nor a .py file"
    return None


def unresolved_source(payload: str) -> str | None:
    if not payload.partition("#")[0].strip():
        return "source reference is empty"
    return None


def unresolved(clause: Clause) -> str | None:
    """Why a well-formed clause's evaluator reference does not resolve, else None."""
    kind, _, payload = clause.header.refs[0].partition(":")
    payload = payload.strip()
    if kind == "test":
        return unresolved_test(payload)
    if kind == "script":
        return unresolved_script(payload)
    return unresolved_source(payload)


def judge_clause(clause: Clause, first_seen: dict[str, Clause]) -> list[Finding]:
    messages = clause.problems
    well_formed = not messages
    if clause.header.ident_ok:
        earlier = first_seen.setdefault(clause.header.ident, clause)
        if earlier is not clause:
            where = f"{earlier.path}:{earlier.line}"
            messages.append(
                f"duplicate clause ID {clause.header.ident!r} (first defined at {where})"
            )
    if well_formed and (reason := unresolved(clause)):
        messages.append(reason)
    return [Finding(clause.path, clause.line, message) for message in messages]


def spec_files(root: Path) -> list[Path]:
    return sorted(root.rglob("*.md")) if root.is_dir() else [root]


def check_command(root: Path) -> int:
    files = spec_files(root)
    findings: list[Finding] = []
    first_seen: dict[str, Clause] = {}
    clauses = 0
    for path in files:
        for clause in scan(str(path), path.read_text(encoding="utf-8")):
            clauses += 1
            findings += judge_clause(clause, first_seen)
    for finding in findings:
        print(finding)
    if findings:
        return 1
    print(f"spec_lint: checked {count(clauses, 'clause')} in {count(len(files), 'file')}")
    return 0


def fixture_args(fixture: Path) -> list[str]:
    """The arguments a fixture runs with: its directive line, else --check on itself."""
    lines = fixture.read_text(encoding="utf-8").splitlines()
    found = DIRECTIVE.fullmatch(lines[0]) if lines else None
    template = found["args"] if found else "--check {file}"
    return [
        word.replace("{file}", str(fixture)).replace("{dir}", str(fixture.parent))
        for word in shlex.split(template)
    ]


def run_fixture(fixture: Path) -> tuple[int, str]:
    """Run one fixture in-process; return its exit code and combined output."""
    args = fixture_args(fixture)
    if "--self-test" in args:
        return 2, "a fixture must not run --self-test\n"
    captured = io.StringIO()
    with contextlib.redirect_stdout(captured), contextlib.redirect_stderr(captured):
        try:
            code = main(args)
        except SystemExit as stop:
            code = stop.code if isinstance(stop.code, int) else 1
    return code, captured.getvalue()


def judge_fixture(fixture: Path) -> list[str]:
    """Compare a fixture's run against its .expect file; return the mismatches."""
    expect = fixture.with_suffix(".expect")
    try:
        lines = expect.read_text(encoding="utf-8").splitlines()
    except OSError:
        return [f"{fixture.name}: no sibling {expect.name}"]
    try:
        want = int(lines[0])
    except (IndexError, ValueError):
        return [f"{expect.name}: the first line must be the expected exit code"]
    green = fixture.name.startswith("green-")
    problems = []
    if green and want != 0:
        problems.append(f"{expect.name}: a green fixture must expect exit 0, not {want}")
    if not green and want == 0:
        problems.append(f"{expect.name}: a red fixture must expect a non-zero exit")
    code, output = run_fixture(fixture)
    if code != want:
        problems.append(f"{fixture.name}: expected exit {want}, got {code}")
    problems += [
        f"{fixture.name}: output lacks {needle!r}"
        for needle in lines[1:]
        if needle.strip() and needle not in output
    ]
    if problems:
        problems += [f"    | {line}" for line in output.splitlines()]
    return problems


def self_test() -> int:
    fixtures = sorted(FIXTURES.glob("*.md"))
    problems = [
        f"{orphan.name}: no sibling {orphan.stem}.md"
        for orphan in sorted(FIXTURES.glob("*.expect"))
        if not orphan.with_suffix(".md").exists()
    ]
    green = sum(fixture.name.startswith("green-") for fixture in fixtures)
    red = len(fixtures) - green
    if red < MIN_RED_FIXTURES:
        problems.append(f"found {red} red fixtures, need at least {MIN_RED_FIXTURES}")
    if green < MIN_GREEN_FIXTURES:
        problems.append(f"found {green} green fixtures, need at least {MIN_GREEN_FIXTURES}")
    for fixture in fixtures:
        problems += judge_fixture(fixture)
    listings = workspace_tests.cache_info().misses
    if listings > 1:
        problems.append(f"`cargo nextest list` ran {listings} times; it must run once per process")
    for problem in problems:
        print(f"self-test: {problem}")
    if problems:
        return 1
    print(f"self-test: {red} red and {green} green fixtures behave as expected")
    return 0


def parse_quotas(spec: str) -> list[Quota]:
    quotas: list[Quota] = []
    for item in spec.split(","):
        found = QUOTA.fullmatch(item)
        if found is None:
            raise UsageError(f"bad coverage entry {item!r}; expected DOMAIN=MIN/VERIFIED")
        if any(quota.domain == found["domain"] for quota in quotas):
            raise UsageError(f"domain {found['domain']!r} is listed twice")
        quotas.append(Quota(found["domain"], int(found["clauses"]), int(found["verified"])))
    return quotas


def shortfall(quota: Quota, directory: Path) -> str | None:
    """Describe how a domain file misses its minimums, or None if it meets them."""
    path = directory / f"{quota.domain}.md"
    clauses = scan(str(path), path.read_text(encoding="utf-8")) if path.is_file() else []
    sound = [clause for clause in clauses if not clause.problems]
    verified = sum(clause.kind in VERIFIED_KINDS for clause in sound)
    gaps = []
    if len(sound) < quota.clauses:
        gaps.append(f"{count(len(sound), 'clause')}, need {quota.clauses}")
    if verified < quota.verified:
        gaps.append(f"{verified} machine-verified, need {quota.verified}")
    if not gaps:
        return None
    notes = []
    if not path.is_file():
        notes.append("file missing")
    if len(clauses) > len(sound):
        notes.append(f"{count(len(clauses) - len(sound), 'malformed clause')} not counted")
    return f"{path}: {'; '.join(gaps)}" + (f" ({', '.join(notes)})" if notes else "")


def table_cells(line: str) -> list[str]:
    body = line.strip().removeprefix("|").removesuffix("|")
    return [cell.strip().replace("\\|", "|") for cell in UNESCAPED_PIPE.split(body)]


def inventory_row(
    number: int, cells: list[str], columns: dict[str, int], width: int
) -> InventoryRow:
    if len(cells) != width:
        return InventoryRow(number, "", (f"row has {len(cells)} cells, expected {width}",))
    name, owner, citation = (cells[columns[key]] for key in INVENTORY_COLUMNS)
    problems = []
    if not name:
        problems.append("missing name")
    if not owner:
        problems.append("missing owner")
    elif owner not in OWNERS:
        problems.append(f"owner {owner!r} is not one of {', '.join(OWNERS)}")
    if not citation:
        problems.append("missing citation")
    return InventoryRow(number, owner, tuple(problems))


def inventory_rows(text: str) -> list[InventoryRow]:
    """The data rows of every table whose header names Name, Owner, and Citation."""
    rows: list[InventoryRow] = []
    header: list[str] | None = None
    columns: dict[str, int] | None = None
    width = 0
    for number, line in live_lines(text):
        if not line.strip().startswith("|"):
            header, columns = None, None
            continue
        cells = table_cells(line)
        if columns is not None:
            rows.append(inventory_row(number, cells, columns, width))
        elif header is not None and all(DELIMITER_CELL.fullmatch(cell) for cell in cells):
            names = [cell.lower() for cell in header]
            if all(key in names for key in INVENTORY_COLUMNS):
                columns = {key: names.index(key) for key in INVENTORY_COLUMNS}
                width = len(header)
        else:
            header = cells
    return rows


def inventory_command(path: Path, minimum: int) -> int:
    rows = inventory_rows(path.read_text(encoding="utf-8"))
    if not rows:
        print(f"{path}: no inventory rows found (expected a table headed Name, Owner, Citation)")
        return 1
    for row in rows:
        for problem in row.problems:
            print(Finding(str(path), row.line, problem))
    engine = sum(row.owner == "engine" and not row.problems for row in rows)
    print(f"spec_lint: {path} has {count(engine, 'engine row')} (minimum {minimum})")
    short = engine < minimum
    if short:
        print(f"{path}: {count(engine, 'engine row')} is below the minimum of {minimum}")
    return 1 if short or any(row.problems for row in rows) else 0


def coverage_command(quotas: list[Quota], directory: Path) -> int:
    gaps = [gap for quota in quotas if (gap := shortfall(quota, directory))]
    for gap in gaps:
        print(gap)
    if gaps:
        return 1
    print(f"spec_lint: coverage met for {count(len(quotas), 'domain')}")
    return 0


def require_command(ident: str, path: Path) -> int:
    clauses = scan(str(path), path.read_text(encoding="utf-8"))
    clauses = [clause for clause in clauses if clause.header.ident == ident]
    for clause in clauses:
        if not clause.problems:
            print(f"spec_lint: found {ident} at {clause.path}:{clause.line}")
            return 0
    for clause in clauses:
        for problem in clause.problems:
            print(Finding(clause.path, clause.line, problem))
    print(f"{path}: no well-formed clause with ID {ident!r}")
    return 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="spec_lint.py",
        description="Lint the SuperCom specification in docs/specs.",
        allow_abbrev=False,
    )
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--check", metavar="PATH", help="check clauses in a file or directory")
    modes.add_argument("--inventory", metavar="FILE", help="check an engine-API inventory table")
    modes.add_argument(
        "--coverage",
        nargs=2,
        metavar=("SPEC", "DIR"),
        help="per-domain minimums, SPEC being DOMAIN=MIN/VERIFIED[,...], counted in DIR/DOMAIN.md",
    )
    modes.add_argument(
        "--require",
        nargs=2,
        metavar=("ID", "FILE"),
        help="succeed only if FILE holds a well-formed clause with this ID",
    )
    modes.add_argument("--self-test", action="store_true", help="run the fixture suite")
    parser.add_argument(
        "--min-engine-rows",
        type=int,
        metavar="N",
        help="with --inventory: fewest engine rows allowed (default 127)",
    )
    return parser


def existing(text: str, kind: str) -> Path:
    """A command-line path that must exist as a file, a directory, or either."""
    path = Path(text)
    found = {"file": path.is_file(), "directory": path.is_dir(), "file or directory": path.exists()}
    if not found[kind]:
        raise UsageError(f"no such {kind}: {text}")
    return path


def dispatch(args: argparse.Namespace) -> int:
    if args.min_engine_rows is not None and args.inventory is None:
        raise UsageError("--min-engine-rows applies only to --inventory")
    if args.self_test:
        return self_test()
    if args.inventory is not None:
        minimum = DEFAULT_MIN_ENGINE_ROWS if args.min_engine_rows is None else args.min_engine_rows
        if minimum < 0:
            raise UsageError("--min-engine-rows must not be negative")
        return inventory_command(existing(args.inventory, "file"), minimum)
    if args.coverage is not None:
        spec, directory = args.coverage
        return coverage_command(parse_quotas(spec), existing(directory, "directory"))
    if args.require is not None:
        ident, file = args.require
        if problems := id_problems(ident):
            raise UsageError(problems[0])
        return require_command(ident, existing(file, "file"))
    return check_command(existing(args.check, "file or directory"))


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return dispatch(args)
    except UsageError as error:
        parser.error(str(error))


if __name__ == "__main__":
    sys.exit(main())
