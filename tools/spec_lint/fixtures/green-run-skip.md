<!-- spec-lint: --run {file} -->

# Green: a SKIP line with exit 0 is a skip, not a failure

**FMT-001** `game-data` `script:tools/spec_lint/fixtures/scripts/report.py 0 stderr "SKIP: the installed game was not found"`
A skip printed to stderr MUST be reported and MUST NOT fail the run.

**FMT-002** `game-data` `script:tools/spec_lint/fixtures/scripts/report.py 0 stdout "SKIP: the installed game was not found"`
A skip printed to stdout is reported the same way.

**FMT-003** `design` `script:tools/spec_lint/fixtures/scripts/report.py 0`
A passing script beside the skips.
