# Default recipe: list available recipes
default:
    @just --list

# Type-check every crate and target
check:
    cargo check --workspace --all-targets

# Run the test suite
test:
    cargo nextest run --workspace
    cargo nextest run -p supercom-detbench

# Lint with the CI warning gate
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Format the whole tree
fmt:
    treefmt

# Fail if any file is not formatted
fmt-check:
    treefmt --fail-on-change

# Check the specification: self-test, every clause, then run its scripts
lint:
    python3 tools/spec_lint.py --self-test
    python3 tools/spec_lint.py --check docs/specs
    python3 tools/spec_lint.py --run docs/specs

# Keep game content out of the tree: self-test the rules, then check every tracked file
scan:
    python3 tools/cleanroom_scan.py --self-test
    python3 tools/cleanroom_scan.py --check

# Audit dependency licenses, bans, and sources (the fixture's packages are fetched by the gate's tests first)
deny:
    cargo deny check licenses bans sources
    cargo deny --manifest-path plugins/hello/Cargo.toml --config deny.toml --locked check licenses bans sources

# Audit dependencies against the advisory database (needs network)
audit:
    cargo deny check advisories

# Run a nextest selection the way CI's hardware jobs do: list it, run it, and fail
# on an empty selection, on a skipped test, or on any listed test that did not pass.
# The arguments are cargo-nextest's package options and its filter expression. A game-dependent test
# skips by printing `SKIP: <reason>` and passing, so a skip line is a failure here.
[private]
ci-select packages filter:
    #!/usr/bin/env bash
    set -euo pipefail
    log=$(mktemp)
    trap 'rm -f "$log"' EXIT
    listed=$(cargo nextest list {{ packages }} -E {{ quote(filter) }} --message-format oneline | grep -c .) || listed=0
    echo "ci-select: the selection lists $listed tests"
    [ "$listed" -gt 0 ] || { echo "ci-select: the selection lists no tests" >&2; exit 1; }
    cargo nextest run {{ packages }} -E {{ quote(filter) }} --no-tests=fail --no-fail-fast 2>&1 | tee "$log"
    passed=$(sed -nE 's/.* ([0-9]+) passed.*/\1/p' "$log" | tail -n 1)
    echo "ci-select: listed $listed, passed ${passed:-0}"
    [ "${passed:-0}" -eq "$listed" ] || { echo "ci-select: not every listed test passed" >&2; exit 1; }
    if grep -E '^[[:space:]]*SKIP:' "$log"; then
        echo "ci-select: a test skipped" >&2
        exit 1
    fi

# GPU tests that need no installed game, on whatever adapter the environment offers
# (CI starts the shell with SUPERCOM_SOFTWARE_VULKAN=1 and sets SUPERCOM_REQUIRE_GPU=1,
# so no adapter fails). A GPU test that needs the installed game carries `installed`
# in its name, so this filter needs no new name when one is added; the two scene
# tests that ask for the game are named out because they predate that rule.
ci-gpu: (ci-select "--workspace" "test(/(^|::)conformance_gpu_/) - test(/installed/) - test(/replay_start_draws|close_zoom_draws/)")

# The determinism and decoding tests on the ARM runner: three packages, without the
# conformance_ tests, which ask for the installed game.
ci-arm: (ci-select "-p supercom-detbench -p supercom-sim -p supercom-dds" "not test(/(^|::)conformance_/)")

# The CI gate: formatting, spec lint, clean-room check, lints, tests, license audit
gate: fmt-check lint scan clippy test deny
