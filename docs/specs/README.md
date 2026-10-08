# SuperCom specification index

The reverse-engineered specification is the authority for the engine: code
satisfies spec clauses, never the other way round. Every clause carries an
identifier and names its evaluator (test, property, contract, or proof), per
predicate's `spec` workflow. The key words MUST, SHOULD, and MAY are to be
read as in BCP 14.

Clean-room rule: the spec records observed behaviour of the installed game and
published, licensed references. It never reproduces game or FAF Lua source.

| Domain      | Scope                                                                                                                                                                           | Status |
| :---------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :----- |
| formats     | `.scd` archives and mount order; the data subset of the GPG Lua dialect in `.bp`; `.scm` and `.sca`; `.scmap` with `_scenario.lua` and `_save.lua`; DDS; XACT `.xwb` and `.xsb` | WIP    |
| data-model  | Nickel contracts for the blueprint families; mod overlay semantics; the import cache                                                                                            | WIP    |
| sim         | tick model, economy, units and categories, orders and build queues, weapons and projectiles, damage and armour, pathfinding, veterancy, intel                                   | WIP    |
| determinism | lockstep command stream, replay format, float strategy, desync detection                                                                                                        | WIP    |
| render      | terrain, decals, props, shadows, water, units, fog of war, strategic icons, post-processing                                                                                     | WIP    |
| plugin-api  | WIT worlds, host imports, versioning, sandbox policy                                                                                                                            | WIP    |
| ai          | the skirmish AI interface and the built-in AI's contract                                                                                                                        | WIP    |
| ui          | basic HUD and input behaviour                                                                                                                                                   | WIP    |

Each domain gets its own file in this directory, authored by the spec campaign.

## Clause grammar

A clause is one paragraph: a header line, then the statement text up to the
first blank line. The clause below is live; `just lint` checks it together with
everything else in this directory.

**FMT-001** `game-data` `script:tools/survey/archive_census.py`
The archive layer MUST enumerate 23,015 files across the 17 archives.

When the formats document adopts this clause as `FMT-001`, delete this copy in
the same change: IDs are unique across the directory.

The header line holds three parts, separated by single spaces:

- **ID**: `<TAG>-<NNN>`, a tag of three or more ASCII letters, a hyphen, and
  three digits. The ID never matches `\b[NPFC][1-9][0-9]*\b`, the pattern of
  planning-node identifiers. A tag need not equal its domain name, so `ai` and
  `ui` take longer tags.
- **Evidence class**, in backticks, one of `game-data` (observed in the
  installed game's data), `reference` (taken from a published, licensed
  source), `engine-stub` (an engine behaviour this project must provide),
  `design` (a decision of this project), or `derived` (follows from other
  clauses).
- **Evaluator reference**, in backticks, exactly one of the forms below.

| Evaluator reference                         | Resolves when                                                                                |
| :------------------------------------------ | :------------------------------------------------------------------------------------------- |
| `test:<name-or-filter>`                     | a line of `cargo nextest list --workspace` holds the text as whole path segments or a suffix |
| `script:<path>[ <args>]`                    | the path, relative to the repository root, is a file that is executable or ends in `.py`     |
| `source:<url-or-archive-member>[#<anchor>]` | the locator before the anchor is not empty; nothing is fetched                               |

For a listed test `pkg::bin/pkg tests::name`, the references `name` and
`tests::name` resolve and `ame` does not. A clause is **machine-verified** when
its evaluator is a `test:` or a `script:`.

A line that starts with `**` is always read as a clause header, so no other
line may begin with bold text. Fenced code and HTML comments are skipped.

### Checking the specification

`tools/spec_lint.py` enforces the grammar with the Python standard library
alone. `just lint` runs its self-test and then `--check docs/specs`, and
`just gate` includes `just lint`. Findings print as `file:line: message`. The
exit status is 0 when clean, 1 on findings, and 2 on a usage error.

| Mode                                               | Succeeds when                                                                                                                                    |
| :------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------- |
| `--check <dir-or-file>`                            | every clause is well-formed, IDs are unique, and every evaluator reference resolves                                                              |
| `--coverage <domain>=<min>/<verified>[,...] <dir>` | each `<dir>/<domain>.md` has at least `<min>` well-formed clauses, at least `<verified>` of them machine-verified; a missing file counts as zero |
| `--inventory <file> [--min-engine-rows <n>]`       | every inventory row is well-formed and there are at least `<n>` engine rows (default 127)                                                        |
| `--require <ID> <file>`                            | the file holds a well-formed clause with exactly that ID                                                                                         |
| `--self-test`                                      | every fixture under `tools/spec_lint/fixtures/` behaves as its `.expect` file says                                                               |

`--coverage` and `--require` check form only: they do not resolve evaluator
references.

An engine-API inventory is a table headed `Name` (a method or behaviour),
`Owner` (`engine` or `lua`), and `Citation`; other columns and other tables are
allowed. Every row fills all three cells.
