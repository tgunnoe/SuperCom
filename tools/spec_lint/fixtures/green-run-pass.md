<!-- spec-lint: --run {file} -->

# Green: script evaluators that exit 0 pass

**FMT-001** `design` `script:tools/spec_lint/fixtures/scripts/report.py 0`
The first sample MUST hold.

**FMT-002** `design` `script:tools/spec_lint/fixtures/scripts/report.py 0 stdout "see SKIP: later"`
A SKIP: marker that does not start a line is ordinary output.

**FMT-003** `design` `source:https://example.com/notes`
Evaluators other than scripts are not run.

**FMT-004** `folklore` `script:tools/spec_lint/fixtures/scripts/report.py 1`
A clause that is not well-formed is left to --check and not run.

**FMT-005** `design` `script:tools/spec_lint/fixtures/scripts/report.sh 0`
An executable script that is not Python runs directly.
