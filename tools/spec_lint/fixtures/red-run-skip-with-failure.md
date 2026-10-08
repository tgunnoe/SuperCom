<!-- spec-lint: --run {file} -->

# Red: a SKIP line does not excuse a non-zero exit

**FMT-001** `game-data` `script:tools/spec_lint/fixtures/scripts/report.py 1 stderr "SKIP: the installed game was not found"`
Only exit 0 turns a SKIP line into a skip.
