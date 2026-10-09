# SuperCom specification index

The reverse-engineered specification is the authority for the engine: code
satisfies spec clauses, never the other way round. Every clause carries an
identifier and names its evaluator: a test, a script, or a pinned source (see
the clause grammar), per predicate's `spec` workflow. The key words MUST,
SHOULD, and MAY are to be read as in BCP 14.

Clean-room rule: the spec records observed behaviour of the installed game and
published references; a reference without a licence is read for understanding
only and quoted at most three lines. It never reproduces game or FAF Lua
source.

| Domain      | Scope                                                                                                                                                                           | Status |
| :---------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :----- |
| formats     | `.scd` archives and mount order; the data subset of the GPG Lua dialect in `.bp`; `.scm` and `.sca`; `.scmap` with `_scenario.lua` and `_save.lua`; DDS; XACT `.xwb` and `.xsb` | WIP    |
| data-model  | Nickel contracts for the blueprint families; mod overlay semantics; the import cache                                                                                            | WIP    |
| sim         | tick model, economy, units and categories, orders and build queues, weapons and projectiles, damage and armour, pathfinding, veterancy, intel                                   | landed |
| determinism | lockstep command stream, replay format, float strategy, desync detection                                                                                                        | WIP    |
| render      | terrain, decals, props, shadows, water, units, fog of war, strategic icons, post-processing                                                                                     | WIP    |
| plugin-api  | WIT worlds, host imports, versioning, sandbox policy                                                                                                                            | WIP    |
| ai          | the skirmish AI interface and the built-in AI's contract                                                                                                                        | WIP    |
| ui          | basic HUD and input behaviour                                                                                                                                                   | WIP    |

Each domain gets its own file in this directory, authored by the spec campaign.

## Clause grammar

A clause is one paragraph: a header line, then the statement text up to the
first blank line. The clause below is live; `just lint` checks it and runs its
evaluator, together with everything else in this directory.

**SPC-001** `design` `script:tools/spec_lint.py --self-test`
The specification lint MUST reject every red fixture and accept every green one.

The header line holds three parts, separated by single spaces:

- **ID**: `<TAG>-<NNN>`, a tag of three or more ASCII letters, a hyphen, and
  three digits. The ID never matches `\b[NPFC][1-9][0-9]*\b`, the pattern of
  planning-node identifiers. A tag need not equal its domain name, so `ai` and
  `ui` take longer tags. IDs are unique across the directory, compared without
  regard to case, so `SPC-001` and `spc-001` collide.
- **Evidence class**, in backticks: where the clause's truth comes from. When
  the installed game and another source both bear on a clause, the installed
  game is the evidence, and it wins where they differ.
  - `game-data`: the installed game's own files (archives and their members,
    such as blueprints, meshes, and Lua; maps; sound banks), read in place by
    a loader or a script.
  - `reference`: a published source other than the game, such as an
    article, a reference implementation, or a format or API document. A
    source without a licence is read for understanding only: cite it by URL
    and line and quote at most three lines.
  - `engine-stub`: the name, arguments, or owner of an engine-exposed
    function or class, as declared by FAF's `engine/` stubs or the
    reconstructed engine header. It fixes the API's shape, not its behaviour.
  - `design`: a decision of this project that the original never exposed
    (the command stream, the replay format, state hashing, the WIT API, a
    budget), or a deviation from the installed game, which cites its ratified
    ADR. A design clause never states how the original behaves.
  - `derived`: follows from other clauses, which its statement names by ID.
    Its evaluator still checks it.
- **Evaluator reference**, in backticks, exactly one of the forms below.

| Evaluator reference           | Resolves when                                                                                |
| :---------------------------- | :------------------------------------------------------------------------------------------- |
| `test:<name-or-filter>`       | a line of `cargo nextest list --workspace` holds the text as whole path segments or a suffix |
| `script:<path>[ <args>]`      | the path, relative to the repository root, is a file that is executable or ends in `.py`     |
| `source:<locator>[#<anchor>]` | the locator before the anchor takes one of the three forms below; nothing is fetched         |

For a listed test `pkg::bin/pkg tests::name`, the references `name` and
`tests::name` resolve and `ame` does not. A clause is **machine-verified** when
its evaluator is a `test:` or a `script:`.

A `test:` or `script:` evaluator MUST fail when its clause is false: a test by
failing, a script by exiting non-zero. A script that only reports a value does
not verify a clause. An evaluator that needs the installed game, which it finds
through `SUPERCOM_FA_DIR`, follows the conformance convention: when the install
is not found it prints `SKIP: <reason>` and succeeds, and with
`SUPERCOM_REQUIRE_GAME=1` it fails instead.

A `source:` locator is one of:

- an `https://` URL. A `github.com` or `raw.githubusercontent.com` URL carries
  a commit hash of 7 to 40 hex digits as its ref (`/blob/<hash>/`,
  `/tree/<hash>/`, or the raw host's `/<owner>/<repo>/<hash>/`), never a branch
  name.
- an install locator `fa:<path under the install>[!<member>]`, such as
  `fa:gamedata/units.scd!units/UEL0001/UEL0001_unit.bp` or
  `fa:bin/SupComDataPath.lua`. The archive is named because member paths are
  not unique across archives.
- a path in this repository that exists, such as
  `docs/adr/0002-data-model-nickel.md`, for design clauses that cite an ADR.

Anything else is a finding. The check is form only: nothing is fetched and the
installed game is never read. An inventory's `Citation` cell is held to the same
rule and must hold at least one locator.

A line that starts with `**` is always read as a clause header, so no other
line may begin with bold text. A header must start at column 0: an indented,
listed, or quoted line that begins with a bold ID and a code span is reported,
not dropped. Fenced code and HTML comments are skipped.

### Checking the specification

`tools/spec_lint.py` enforces the grammar with the Python standard library
alone. `just lint` runs its self-test, then `--check docs/specs`, then
`--run docs/specs`, and `just gate` includes `just lint`. Findings print as
`file:line: message`. The exit status is 0 when clean, 1 on findings, and 2 on a
usage error.

| Mode                                               | Succeeds when                                                                                                                                    |
| :------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------- |
| `--check <dir-or-file>`                            | every clause is well-formed, IDs are unique, and every evaluator reference resolves                                                              |
| `--run <dir-or-file> [--timeout <seconds>]`        | no `script:` evaluator of a well-formed clause fails when run from the repository root                                                           |
| `--coverage <domain>=<min>/<verified>[,...] <dir>` | each `<dir>/<domain>.md` has at least `<min>` well-formed clauses, at least `<verified>` of them machine-verified; a missing file counts as zero |
| `--inventory <file> [--min-engine-rows <n>]`       | every inventory row is well-formed and there are at least `<n>` engine rows (default 127)                                                        |
| `--require <ID> <file>`                            | the file holds a well-formed clause with exactly that ID                                                                                         |
| `--self-test`                                      | every fixture under `tools/spec_lint/fixtures/` behaves as its `.expect` file says                                                               |

`--coverage` and `--require` check form only: they do not resolve evaluator
references.

`--run` executes each `script:` evaluator from the repository root and passes its
own environment through, so `SUPERCOM_FA_DIR` and `SUPERCOM_REQUIRE_GAME` reach
the script. Exit 0 is a pass. Exit 0 with a line of output that starts with
`SKIP:` is a skip, which is reported and does not fail the run. Anything else is
a failure, as is an evaluator still running after `--timeout` seconds (default
1800). It prints one line per evaluator with its status, the tail of a failed
evaluator's output under that line, and a summary. Only well-formed clauses run;
`test:` evaluators run under `just test`, and a `source:` evaluator has nothing
to run.

An engine-API inventory is a table headed `Name` (a method or behaviour),
`Owner` (`engine` or `lua`), and `Citation`; other columns and other tables are
allowed. Every row fills all three cells, and its `Citation` holds at least one
locator.
