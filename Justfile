# Default recipe: list available recipes
default:
    @just --list

# Type-check every crate and target
check:
    cargo check --workspace --all-targets

# Run the test suite
test:
    cargo nextest run --workspace

# Lint with the CI warning gate
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Format the whole tree
fmt:
    treefmt

# Fail if any file is not formatted
fmt-check:
    treefmt --fail-on-change

# Audit dependency licenses, bans, and sources (no network needed)
deny:
    cargo deny check licenses bans sources

# Audit dependencies against the advisory database (needs network)
audit:
    cargo deny check advisories

# The CI gate: formatting, lints, tests, license audit
gate: fmt-check clippy test deny
