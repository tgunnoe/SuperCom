<!-- spec-lint: --check {file} --design-exempt {dir}/exempt-test-refs.txt -->

# Green: test references resolve against the workspace test list

**FMT-001** `design` `test:override_wins_over_home`
A bare test name resolves as a whole path segment.

**FMT-002** `design` `test:tests::override_wins_over_home`
A module-qualified suffix resolves as well.

**FMT-003** `design` `test:supercom::bin/supercom tests::override_wins_over_home`
A whole listing line resolves.
