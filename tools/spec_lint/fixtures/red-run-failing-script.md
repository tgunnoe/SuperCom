<!-- spec-lint: --run {file} -->

# Red: a script that exits non-zero fails the run

**FMT-001** `design` `script:tools/spec_lint/fixtures/scripts/report.py 0`
This passing sample runs first.

**FMT-002** `design` `script:tools/spec_lint/fixtures/scripts/report.py 1 stderr boom`
The failing sample MUST be reported with its output.

**FMT-003** `design` `script:tools/spec_lint/fixtures/scripts/report.py 0`
The run MUST continue after a failure.
