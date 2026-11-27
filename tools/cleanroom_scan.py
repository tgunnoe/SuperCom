#!/usr/bin/env python3
"""Keep game and Forged Alliance content out of the SuperCom tree. Standard library only.

Usage (exactly one mode per run):
  cleanroom_scan.py --check [--install DIR]   scan every tracked path
  cleanroom_scan.py --self-test               run every rule on synthesized and committed inputs

Exit status: 0 clean, 1 findings (printed as file:line: message), 2 usage error or a scan
that could not run.

--check lists the tracked paths with `git ls-files`, run from the repository root, and
reports one finding per line when:
  - a tracked path ends in a game file type (DENIED_EXTENSIONS), in any letter case;
  - a tracked file under any directory named fixtures is larger than 4,096 bytes;
  - a tracked text file shares a shingle with the installed game's text (the overlap rule).

The overlap rule. A tracked file is text when its bytes decode as UTF-8 and its first 8,192
bytes hold no NUL. A line is normalized by stripping it and collapsing internal whitespace
to one space; empty lines and lines shorter than four characters are dropped. A shingle is
four consecutive normalized lines, so a quotation of three lines or fewer never matches. The
game's text is every member of gamedata/*.scd whose name ends in .bp, .lua, .fx or .cg
(decoded as ISO-8859-1) plus every maps/*/*.lua file. A text file is reported once, at the
line where its earliest matching run starts, with the length of that run.

The shingle is its four normalized lines joined by newlines and is used as the key itself:
equality is exact, with no digest and no collisions. Only the tracked shingles are held in
memory; the game's text is streamed past them one file at a time.

The install is --install DIR when given, else SUPERCOM_FA_DIR, and nothing else. When it
names no install (unset, not a directory, or no gamedata/*.scd inside) the overlap scan is
skipped, exactly one WARNING line goes to stderr, and the other rules still decide the exit
status. An install that exists but cannot be read ends the run with status 2.

The scan reads archive members in memory, one at a time, and writes nothing. It prints
tracked paths, line numbers, and counts, never any content of the game.

--self-test runs every rule against a synthetic install and synthetic trees built in a
temporary directory, and against the green fixtures under tools/cleanroom_scan/fixtures/. A
red input, one that must violate a rule, is built in code and never committed; the self-test
refuses a committed fixture that is not green. It never reads the real game.
"""

import argparse
import contextlib
import io
import os
import stat
import subprocess
import sys
import tempfile
import zipfile
import zlib
from collections.abc import Callable, Iterator, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path

TOOL_DIR = Path(__file__).resolve().parent
REPO_ROOT = TOOL_DIR.parent
FIXTURES = TOOL_DIR / "cleanroom_scan" / "fixtures"

INSTALL_VAR = "SUPERCOM_FA_DIR"
WARNING = "WARNING: SUPERCOM_FA_DIR is unset or names no install; the overlap scan was skipped"

# File types the game ships, and the image formats a screenshot or captured frame is
# written in. None may be tracked, in any letter case.
DENIED_EXTENSIONS = frozenset(
    (
        "bp scm sca scmap dds xwb xsb sfd fx cg lua scd tga bmp psd txmap ogg wav "
        "png ppm pnm pam jpg jpeg"
    ).split()
)
FIXTURE_DIR = "fixtures"
FIXTURE_LIMIT = 4096

TEXT_PROBE = 8192
MIN_LINE = 4
SHINGLE = 4
CORPUS_SUFFIXES = (".bp", ".lua", ".fx", ".cg")
ARCHIVE_ERRORS = (
    OSError,
    zipfile.BadZipFile,
    NotImplementedError,
    RuntimeError,
    EOFError,
    zlib.error,
)

MIN_FIXTURES = 2


class UsageError(Exception):
    """The command line is wrong; reported through argparse as exit status 2."""


class ScanError(Exception):
    """The scan could not run; reported on stderr as exit status 2."""


@dataclass(frozen=True, slots=True)
class Finding:
    path: str
    line: int
    rule: str
    message: str

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.message}"


@dataclass(slots=True)
class Candidate:
    """A tracked text file and which of its shingles the game's text holds."""

    path: str
    numbers: list[int]  # the source line number of each surviving line
    matched: set[int] = field(default_factory=set)  # indexes of matching shingles


@dataclass(slots=True)
class Report:
    findings: list[Finding] = field(default_factory=list)
    files: int = 0  # tracked paths checked
    compared: int = 0  # tracked text files compared with the game's text
    corpus: int = 0  # installed game files read


def count(number: int, noun: str) -> str:
    return f"{number} {noun}" if number == 1 else f"{number} {noun}s"


def denied_extension(path: str) -> str | None:
    """The denylisted extension the file name ends in, in any letter case, else None."""
    _, dot, extension = path.rsplit("/", 1)[-1].lower().rpartition(".")
    return extension if dot and extension in DENIED_EXTENSIONS else None


def under_fixtures(path: str) -> bool:
    """Whether a directory on the path, not the file itself, is named fixtures."""
    return FIXTURE_DIR in path.split("/")[:-1]


def decode_text(data: bytes) -> str | None:
    """The text of a file, or None when it is binary: a NUL early on, or not UTF-8."""
    if b"\0" in data[:TEXT_PROBE]:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def split_lines(text: str) -> list[str]:
    """Lines split on \\n, \\r\\n, or \\r alone and nothing else, so a stray control
    character in a game file cannot move a line boundary."""
    return text.replace("\r\n", "\n").replace("\r", "\n").split("\n")


def numbered_lines(text: str) -> Iterator[tuple[int, str]]:
    """Each normalized line that survives the drop rules, with its 1-based line number."""
    for number, raw in enumerate(split_lines(text), 1):
        line = " ".join(raw.split())
        if len(line) >= MIN_LINE:
            yield number, line


class Needles:
    """The shingles of the tracked text files, waiting to be found in the game's text."""

    def __init__(self) -> None:
        self.candidates: list[Candidate] = []
        self.lines: set[str] = set()  # every surviving tracked line: a cheap pre-filter
        self.shingles: dict[str, list[tuple[Candidate, int]]] = {}

    def add(self, path: str, text: str) -> None:
        numbered = list(numbered_lines(text))
        if len(numbered) < SHINGLE:
            return
        lines = [line for _, line in numbered]
        candidate = Candidate(path, [number for number, _ in numbered])
        self.candidates.append(candidate)
        self.lines.update(lines)
        for index in range(len(lines) - SHINGLE + 1):
            key = "\n".join(lines[index : index + SHINGLE])
            self.shingles.setdefault(key, []).append((candidate, index))

    def probe(self, text: str) -> None:
        """Record every tracked shingle that this piece of the game's text holds."""
        lines = [line for _, line in numbered_lines(text)]
        run = 0  # consecutive lines, ending here, that some tracked file also has
        for index, line in enumerate(lines):
            run = run + 1 if line in self.lines else 0
            if run >= SHINGLE:
                key = "\n".join(lines[index - SHINGLE + 1 : index + 1])
                for candidate, at in self.shingles.get(key, ()):
                    candidate.matched.add(at)

    def findings(self) -> list[Finding]:
        """One finding per file, at the line where its earliest matching run starts."""
        found = []
        for candidate in self.candidates:
            if not candidate.matched:
                continue
            first = last = min(candidate.matched)
            while last + 1 in candidate.matched:
                last += 1
            length = last - first + SHINGLE
            message = f"{length} consecutive lines match the installed game's files"
            found.append(Finding(candidate.path, candidate.numbers[first], "overlap", message))
        return found


def archive_paths(install: Path) -> list[Path]:
    directory = install / "gamedata"
    return sorted(directory.glob("*.scd")) if directory.is_dir() else []


def map_scripts(install: Path) -> list[Path]:
    maps = install / "maps"
    if not maps.is_dir():
        return []
    return sorted(
        script
        for directory in maps.iterdir()
        if directory.is_dir()
        for script in directory.iterdir()
        if script.is_file() and script.name.lower().endswith(".lua")
    )


def corpus_texts(install: Path) -> Iterator[str]:
    """The game's text one file at a time, in memory: archive members, then map scripts.

    A failure names the archive or the map folder and the exception class, never content."""
    for archive in archive_paths(install):
        try:
            with zipfile.ZipFile(archive) as bundle:
                for info in bundle.infolist():
                    if not info.is_dir() and info.filename.lower().endswith(CORPUS_SUFFIXES):
                        yield bundle.read(info).decode("iso-8859-1")
        except ARCHIVE_ERRORS as error:
            detail = f"archive {archive.name} ({type(error).__name__})"
            raise ScanError(f"cannot read installed {detail}") from error
    try:
        scripts = map_scripts(install)
    except OSError as error:
        raise ScanError(f"cannot list installed maps ({type(error).__name__})") from error
    for script in scripts:
        try:
            yield script.read_bytes().decode("iso-8859-1")
        except OSError as error:
            detail = f"map script in {script.parent.name} ({type(error).__name__})"
            raise ScanError(f"cannot read installed {detail}") from error


def resolve_install(argument: str | None, environ: Mapping[str, str]) -> Path | None:
    """The install directory from --install, else SUPERCOM_FA_DIR; None when it holds no game."""
    text = argument if argument is not None else environ.get(INSTALL_VAR, "")
    if not text:
        return None
    path = Path(text)
    return path if archive_paths(path) else None


def tracked_paths(root: Path, env: Mapping[str, str] | None = None) -> list[str]:
    """Every path git tracks under root, listed from root with NUL separators."""
    try:
        done = subprocess.run(
            ["git", "ls-files", "-z"], cwd=root, env=env, capture_output=True, check=False
        )
    except OSError as error:
        raise ScanError(f"cannot run git ({type(error).__name__})") from error
    if done.returncode != 0:
        raise ScanError(f"git ls-files failed with status {done.returncode}")
    return [os.fsdecode(raw) for raw in done.stdout.split(b"\0") if raw]


def repo_tree() -> tuple[Path, list[str]]:
    return REPO_ROOT, tracked_paths(REPO_ROOT)


def scan_tree(root: Path, paths: Sequence[str], install: Path | None) -> Report:
    """Apply every rule to the named files under root; compare text with install if given."""
    report = Report(files=len(paths))
    needles = Needles() if install is not None else None
    for path in paths:
        extension = denied_extension(path)
        if extension is not None:
            message = f"file type .{extension} is a game format and must not be tracked"
            report.findings.append(Finding(path, 1, "extension", message))
        target = root / path
        try:
            status = target.lstat()
        except FileNotFoundError:
            continue  # tracked but deleted from the working tree: nothing to read
        except OSError as error:
            raise ScanError(f"cannot stat {path} ({type(error).__name__})") from error
        if not stat.S_ISREG(status.st_mode):
            continue
        if under_fixtures(path) and status.st_size > FIXTURE_LIMIT:
            message = f"{status.st_size} bytes exceeds the {FIXTURE_LIMIT}-byte limit for fixtures"
            report.findings.append(Finding(path, 1, "fixture-size", message))
        if needles is None:
            continue
        try:
            data = target.read_bytes()
        except OSError as error:
            raise ScanError(f"cannot read {path} ({type(error).__name__})") from error
        text = decode_text(data)
        if text is not None:
            report.compared += 1
            needles.add(path, text)
    if install is not None and needles is not None and needles.candidates:
        for text in corpus_texts(install):
            report.corpus += 1
            needles.probe(text)
        report.findings += needles.findings()
    report.findings.sort(key=lambda finding: (finding.path, finding.line, finding.rule))
    return report


def check_command(root: Path, paths: Sequence[str], install: Path | None) -> int:
    if install is None:
        print(WARNING, file=sys.stderr)
    report = scan_tree(root, paths, install)
    for finding in report.findings:
        print(finding)
    if report.findings:
        return 1
    checked = f"checked {count(report.files, 'tracked file')}"
    if install is None:
        print(f"cleanroom_scan: {checked}; the overlap scan did not run")
    else:
        compared = f"compared {count(report.compared, 'text file')}"
        print(f"cleanroom_scan: {checked}, {compared} with {count(report.corpus, 'game file')}")
    return 0


# --- the self-test: invented game text, trees, and installs -----------------------------

# Every line below is invented for this test. None of it comes from the game.
SYNTH_LUA = (
    "-- invented text for the clean-room scan self-test",
    "synthetic_ledger = {}",
    'synthetic_ledger.alpha = "the first invented entry"',
    'synthetic_ledger.bravo = "the second invented entry"',
    'synthetic_ledger.charlie = "the third invented entry"',
    'synthetic_ledger.delta = "the fourth invented entry"',
    'synthetic_ledger.echo = "the fifth invented entry"',
    "stop",
    "return synthetic_ledger",
)
SYNTH_BP = (
    "SyntheticBlueprint {",
    '    Description = "invented caf\xe9 blueprint",',
    '    Display = { Label = "invented display label" },',
    "    Economy = { BuildCostEnergy = 1111, BuildCostMass = 2222 },",
    '    General = { Tag = "invented general tag" },',
    "}",
)
SYNTH_FX = (
    "// invented shader text for the scan self-test",
    "technique SyntheticTechnique",
    "{",
    "    pass SyntheticPass",
    "    {",
    "        VertexShader = compile vs_2_0 SyntheticVertex();",
    "        PixelShader = compile ps_2_0 SyntheticPixel();",
    "    }",
    "}",
)
SYNTH_CG = (
    "// invented cg text for the scan self-test",
    "float4 SyntheticMain(float2 coord : TEXCOORD0) : COLOR",
    "{",
    "    float4 value = float4(coord, 0.0, 1.0);",
    "    return value * float4(0.25, 0.5, 0.75, 1.0);",
    "}",
)
SYNTH_NOTES = (
    "Invented notes that ship in the synthetic archive but are neither script nor shader text.",
    "A tracked copy of these lines must never match, because the member name is not in the corpus.",
    "The third invented note line is here so that four lines exist.",
    "The fourth invented note line completes the shingle.",
)
SYNTH_MAP = (
    "-- invented map text for the clean-room scan self-test",
    "synthetic_map = {",
    '    save_entry = "invented save entry one",',
    '    save_entry_extra = "invented save entry two",',
    '    save_marker = "invented save marker",',
    '    save_region = "invented save region",',
    "}",
)
SYNTH_MEMBERS = {
    "lua/synthetic/ledger.lua": SYNTH_LUA,
    "units/synthetic/synthetic_unit.bp": SYNTH_BP,
    "effects/synthetic.fx": SYNTH_FX,
    "effects/SYNTHETIC.CG": SYNTH_CG,
    "docs/synthetic_notes.txt": SYNTH_NOTES,
}
SYNTH_MAP_PATH = "maps/synth/synth_save.lua"
NOTE = (
    "a first line of ordinary prose",
    "a second line of ordinary prose",
    "a third line of ordinary prose",
)
# The file types the specification forbids, spelled out here so the check does not
# depend on the constant it checks.
SPEC_DENIED = "bp scm sca scmap dds xwb xsb sfd fx cg lua scd tga bmp psd txmap ogg wav".split()
# Rendered images (screenshots, captured frames) are refused by extension too, one case each.
IMAGE_DENIED = "png ppm pnm pam jpg jpeg".split()


def body(*lines: str) -> bytes:
    """A tracked file's bytes: the lines, UTF-8 encoded, ending in a newline."""
    return ("\n".join(lines) + "\n").encode("utf-8")


def member_bytes(lines: Sequence[str]) -> bytes:
    """An installed file's bytes: the lines, ISO-8859-1 encoded, ending in a newline."""
    return ("\n".join(lines) + "\n").encode("iso-8859-1")


def write_tree(root: Path, files: Mapping[str, bytes]) -> None:
    for name, data in files.items():
        target = root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)


def build_install(directory: Path) -> Path:
    """A synthetic install: one stored archive of invented members and one map script."""
    archive = io.BytesIO()
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_STORED) as bundle:
        for name, lines in SYNTH_MEMBERS.items():
            bundle.writestr(name, member_bytes(lines))
    write_tree(
        directory,
        {"gamedata/synthetic.scd": archive.getvalue(), SYNTH_MAP_PATH: member_bytes(SYNTH_MAP)},
    )
    return directory


@dataclass(frozen=True, slots=True)
class TreeCase:
    """Files to scan against the synthetic install and the findings that must result."""

    name: str
    files: Mapping[str, bytes]
    expect: tuple[tuple[str, int, str], ...] = ()  # (path, line, rule); empty means green
    says: str = ""  # a phrase some finding's message must hold


@dataclass(frozen=True, slots=True)
class CliCase:
    """A run of main() on a tree, with {install}, {absent}, {empty} and {broken} filled in."""

    name: str
    argv: tuple[str, ...]
    files: Mapping[str, bytes]
    environ: Mapping[str, str]
    code: int
    stderr: str | None = ""  # the exact stderr; None leaves it unchecked
    stdout: str = ""  # a phrase stdout must hold


def tree_cases() -> list[TreeCase]:
    copy = "notes/copy.txt"
    lua_run = SYNTH_LUA[2:6]  # four consecutive lines of the invented Lua member

    def nul_at(offset: int) -> bytes:
        head = body(*lua_run)
        return head + b"#" * (offset - len(head)) + b"\0\n"

    denied = {f"assets/probe.{ext}": b"x\n" for ext in SPEC_DENIED}
    denied |= {f"assets/PROBE.{ext.upper()}": b"x\n" for ext in SPEC_DENIED}
    messy = (
        b"\r\n"
        b'\t  synthetic_ledger.alpha   =  "the first invented entry"\r\n'
        b"\r\n"
        b"end\r\n"
        b' synthetic_ledger.bravo = "the second invented entry"   \r\n'
        b"}\r\n"
        b'synthetic_ledger.charlie =\t"the third invented entry"\r'
        b'synthetic_ledger.delta = "the fourth invented entry"\r\n'
    )
    lookalikes = [
        "docs/lua.md",
        "src/luau.rs",
        "notes/bp",
        "notes/cg.txt",
        "a/b.bp.md",
        "x.lua.txt",
        "scmap_notes.txt",
        ".gitignore",
        "docs/wav",
        "tools/bpx.py",
    ]
    return [
        TreeCase(
            "denylisted-extensions-any-case", denied, tuple((p, 1, "extension") for p in denied)
        ),
        *(
            TreeCase(
                f"image-extension-{ext}",
                {f"shots/frame.{ext}": b"x\n", f"shots/FRAME.{ext.upper()}": b"x\n"},
                ((f"shots/frame.{ext}", 1, "extension"), (f"shots/FRAME.{ext.upper()}", 1, "extension")),
            )
            for ext in IMAGE_DENIED
        ),
        TreeCase("extension-lookalikes", {name: b"x\n" for name in lookalikes}),
        TreeCase(
            "fixtures-over-the-limit",
            {"fixtures/big.txt": b"x" * 4097, "crate/tests/fixtures/deep/big.txt": b"y" * 5000},
            (
                ("crate/tests/fixtures/deep/big.txt", 1, "fixture-size"),
                ("fixtures/big.txt", 1, "fixture-size"),
            ),
        ),
        TreeCase(
            "fixtures-at-the-limit",
            {
                "fixtures/edge.txt": b"x" * 4096,
                "docs/large.txt": b"x" * 9000,
                "src/fixtures.rs": b"x" * 5000,
                "notes/fixtures": b"x" * 5000,
            },
        ),
        TreeCase(
            "overlap-four-lines-of-a-script",
            {copy: body(NOTE[0], NOTE[1], *lua_run, NOTE[2])},
            ((copy, 3, "overlap"),),
            "4 consecutive lines",
        ),
        TreeCase(
            "overlap-blueprint-decoded-as-latin-1",
            {copy: body(*SYNTH_BP[:4])},
            ((copy, 1, "overlap"),),
        ),
        TreeCase(
            "overlap-shader-ignoring-short-lines",
            {copy: body(*SYNTH_FX)},
            ((copy, 1, "overlap"),),
            "5 consecutive lines",
        ),
        TreeCase(
            "overlap-member-with-an-uppercase-suffix",
            {copy: body(*SYNTH_CG)},
            ((copy, 1, "overlap"),),
        ),
        TreeCase(
            "overlap-map-script-on-disk",
            {copy: body(NOTE[0], *SYNTH_MAP[2:6])},
            ((copy, 2, "overlap"),),
        ),
        TreeCase("overlap-after-normalizing", {copy: messy}, ((copy, 2, "overlap"),)),
        TreeCase(
            "overlap-reported-once-at-the-first-run",
            {copy: body(NOTE[0], *lua_run, NOTE[1], NOTE[2], *SYNTH_BP[:4])},
            ((copy, 2, "overlap"),),
        ),
        TreeCase(
            "overlap-with-a-nul-beyond-the-probe", {copy: nul_at(8192)}, ((copy, 1, "overlap"),)
        ),
        TreeCase("three-line-quote-never-matches", {copy: body(NOTE[0], *SYNTH_LUA[2:5], NOTE[1])}),
        TreeCase(
            "four-lines-broken-by-another",
            {copy: body(*SYNTH_LUA[2:4], NOTE[0], *SYNTH_LUA[4:6])},
        ),
        TreeCase(
            "four-character-line-is-kept",
            {copy: body(SYNTH_LUA[4], SYNTH_LUA[5], SYNTH_LUA[6], SYNTH_LUA[8])},
        ),
        TreeCase("nul-inside-the-probe-is-binary", {copy: nul_at(8191)}),
        TreeCase("invalid-utf-8-is-not-text", {copy: body(*lua_run) + b"\xff\xfe\n"}),
        TreeCase("member-outside-the-corpus-suffixes", {copy: body(*SYNTH_NOTES)}),
    ]


def cli_cases() -> list[CliCase]:
    clean = {"notes/clean.txt": body(*NOTE)}
    dirty = {"assets/probe.lua": b"x\n"}
    copy = {"notes/copy.txt": body(*SYNTH_LUA[2:6])}
    check = ("--check",)
    # The specification's wording, spelled out so the check does not use WARNING.
    warn = (
        "WARNING: SUPERCOM_FA_DIR is unset or names no install; "
        "the overlap scan was skipped\n"
    )
    var = INSTALL_VAR
    usage = [
        ("no-mode", ()),
        ("two-modes", ("--check", "--self-test")),
        ("unknown-flag", ("--bogus",)),
        ("install-with-self-test", ("--self-test", "--install", "{install}")),
    ]
    return [
        CliCase("unset-variable-warns-once", check, clean, {}, 0, warn),
        CliCase("missing-directory-warns-once", check, clean, {var: "{absent}"}, 0, warn),
        CliCase("directory-without-archives-warns-once", check, clean, {var: "{empty}"}, 0, warn),
        CliCase("installed-variable-is-silent", check, clean, {var: "{install}"}, 0, ""),
        CliCase(
            "flag-beats-variable",
            ("--check", "--install", "{install}"),
            clean,
            {var: "{absent}"},
            0,
            "",
        ),
        CliCase(
            "absent-flag-beats-variable",
            ("--check", "--install", "{absent}"),
            clean,
            {var: "{install}"},
            0,
            warn,
        ),
        CliCase("warning-keeps-the-path-rules", check, dirty, {}, 1, warn, "assets/probe.lua:1:"),
        CliCase("overlap-exits-one", check, copy, {var: "{install}"}, 1, "", "notes/copy.txt:1:"),
        CliCase(
            "unreadable-archive-exits-two",
            check,
            copy,
            {var: "{broken}"},
            2,
            "cleanroom_scan: cannot read installed archive broken.scd (BadZipFile)\n",
        ),
        *(CliCase(f"usage-{name}", argv, clean, {}, 2, None) for name, argv in usage),
    ]


def run_main(
    argv: Sequence[str], tree: Callable[[], tuple[Path, list[str]]], environ: Mapping[str, str]
) -> tuple[int, str, str]:
    """Run main() in-process; return its exit code, stdout, and stderr."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        try:
            code = main(argv, tree=tree, environ=environ)
        except SystemExit as stop:
            code = stop.code if isinstance(stop.code, int) else 1
    return code, out.getvalue(), err.getvalue()


def judge_tree(case: TreeCase, scratch: Path, install: Path) -> list[str]:
    root = scratch / "tree" / case.name
    write_tree(root, case.files)
    report = scan_tree(root, sorted(case.files), install)
    got = [(finding.path, finding.line, finding.rule) for finding in report.findings]
    problems = []
    if got != sorted(case.expect):
        problems.append(f"{case.name}: expected {sorted(case.expect)}, got {got}")
    if case.says and not any(case.says in finding.message for finding in report.findings):
        problems.append(f"{case.name}: no finding says {case.says!r}")
    return problems


def judge_cli(case: CliCase, scratch: Path, places: Mapping[str, str]) -> list[str]:
    root = scratch / "cli" / case.name
    write_tree(root, case.files)
    argv = [word.format(**places) for word in case.argv]
    environ = {key: value.format(**places) for key, value in case.environ.items()}
    code, out, err = run_main(argv, lambda: (root, sorted(case.files)), environ)
    problems = []
    if code != case.code:
        problems.append(f"{case.name}: expected exit {case.code}, got {code}")
    if case.stderr is not None and err != case.stderr:
        problems.append(f"{case.name}: expected stderr {case.stderr!r}, got {err!r}")
    if case.stdout not in out:
        problems.append(f"{case.name}: stdout lacks {case.stdout!r}")
    return problems


def quotes_member(lines: Sequence[str], member: Sequence[str], length: int) -> bool:
    """Whether lines hold a run of this many consecutive lines that member also holds."""
    runs = {tuple(member[i : i + length]) for i in range(len(member) - length + 1)}
    return any(tuple(lines[i : i + length]) in runs for i in range(len(lines) - length + 1))


def judge_fixtures(install: Path, directory: Path) -> tuple[list[str], int]:
    """The committed fixtures must all be green: small, clean, and not vacuous."""
    files = sorted(path for path in directory.glob("*") if path.is_file())
    problems = []
    if len(files) < MIN_FIXTURES:
        problems.append(f"found {len(files)} fixtures, need at least {MIN_FIXTURES}")
    for path in files:
        if not path.name.startswith("green-"):
            problems.append(f"{path.name}: a committed fixture must be green; build red in code")
        scanned = scan_tree(directory.parent, [f"{directory.name}/{path.name}"], install)
        for finding in scanned.findings:
            problems.append(f"{path.name}: {finding.rule} finding: {finding.message}")
    quote = directory / "green-three-line-quote.txt"
    if not quote.is_file():
        problems.append("green-three-line-quote.txt: missing")
    else:
        lines = [line for _, line in numbered_lines(quote.read_text(encoding="utf-8"))]
        member = [line for _, line in numbered_lines("\n".join(SYNTH_LUA))]
        if not quotes_member(lines, member, 3):
            problems.append("green-three-line-quote.txt: no longer quotes the synthetic member")
        if quotes_member(lines, member, 4):
            problems.append("green-three-line-quote.txt: quotes four lines, so it is not green")
    return problems, len(files)


def judge_fixture_guard(install: Path, scratch: Path) -> list[str]:
    """The fixture guard must reject a red name, an oversize file, and an overlapping file."""
    directory = scratch / "guard" / FIXTURE_DIR
    write_tree(directory, {"red-copy.txt": body(*SYNTH_LUA[2:6]), "green-big.txt": b"x" * 4097})
    problems, _ = judge_fixtures(install, directory)
    empty = scratch / "guard-empty" / FIXTURE_DIR
    empty.mkdir(parents=True)
    problems += judge_fixtures(install, empty)[0]
    wanted = (
        "red-copy.txt: a committed fixture must be green",
        "red-copy.txt: overlap finding",
        "green-big.txt: fixture-size finding",
        "need at least",
        "green-three-line-quote.txt: missing",
    )
    return [
        f"fixture guard: no problem mentions {phrase!r}"
        for phrase in wanted
        if not any(phrase in problem for problem in problems)
    ]


def judge_listing(scratch: Path) -> list[str]:
    """tracked_paths lists the tracked files and only those, odd names included."""
    repo = scratch / "listing"
    odd = "spaced dir/caf\xe9 file.txt"
    write_tree(repo, {odd: b"x\n", "kept.txt": b"x\n", "loose.txt": b"x\n"})
    local = subprocess.run(  # the variables that would aim git at some other repository
        ["git", "rev-parse", "--local-env-vars"], capture_output=True, text=True, check=False
    ).stdout.split()
    env = {key: value for key, value in os.environ.items() if key not in local}
    env |= {"GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
    for args in (["init", "-q"], ["add", "--", odd, "kept.txt"]):
        done = subprocess.run(["git", *args], cwd=repo, env=env, capture_output=True, check=False)
        if done.returncode != 0:
            return [f"listing: git {args[0]} exited {done.returncode}"]
    got = sorted(tracked_paths(repo, env))
    want = sorted([odd, "kept.txt"])
    return [] if got == want else [f"listing: expected {want}, got {got}"]


def self_test() -> int:
    """Run every case against synthetic inputs; never read the real game."""
    problems: list[str] = []
    red = green = 0
    with tempfile.TemporaryDirectory() as directory:
        scratch = Path(directory)
        install = build_install(scratch / "install")
        (scratch / "empty").mkdir()
        write_tree(scratch / "broken", {"gamedata/broken.scd": b"this is not a zip archive\n"})
        places = {key: str(scratch / key) for key in ("absent", "empty", "broken")}
        places["install"] = str(install)
        for tree_case in tree_cases():
            problems += judge_tree(tree_case, scratch, install)
            red, green = (red + 1, green) if tree_case.expect else (red, green + 1)
        for cli_case in cli_cases():
            problems += judge_cli(cli_case, scratch, places)
            red, green = (red + 1, green) if cli_case.code else (red, green + 1)
        fixture_problems, fixtures = judge_fixtures(install, FIXTURES)
        problems += fixture_problems + judge_fixture_guard(install, scratch)
        problems += judge_listing(scratch)
        red += 1
        green += fixtures + 1
    for problem in problems:
        print(f"self-test: {problem}")
    if problems:
        return 1
    print(f"self-test: {red} red and {green} green cases behave as expected")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="cleanroom_scan.py",
        description="Keep game and Forged Alliance content out of the SuperCom tree.",
        allow_abbrev=False,
    )
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--check", action="store_true", help="scan every tracked path")
    modes.add_argument("--self-test", action="store_true", help="run the rules on synthetic inputs")
    parser.add_argument(
        "--install",
        metavar="DIR",
        help=f"with --check: the game install to compare with (default: ${INSTALL_VAR})",
    )
    return parser


def dispatch(
    args: argparse.Namespace,
    tree: Callable[[], tuple[Path, list[str]]],
    environ: Mapping[str, str],
) -> int:
    if args.self_test:
        if args.install is not None:
            raise UsageError("--install applies only to --check")
        return self_test()
    install = resolve_install(args.install, environ)
    root, paths = tree()
    return check_command(root, paths, install)


def main(
    argv: Sequence[str] | None = None,
    *,
    tree: Callable[[], tuple[Path, list[str]]] = repo_tree,
    environ: Mapping[str, str] | None = None,
) -> int:
    """Run the tool. tree and environ are seams for the self-test; the CLI never sets them."""
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return dispatch(args, tree, os.environ if environ is None else environ)
    except UsageError as error:
        parser.error(str(error))
    except ScanError as error:
        print(f"cleanroom_scan: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
