#!/usr/bin/env bash
# Prove that crates/sim/clippy.toml bites: a platform transcendental and a
# filesystem read in non-test code fail `cargo clippy -D warnings`, and an
# unwrap inside a test passes. Exits 0 only when all three behave so.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export CARGO_TARGET_DIR="$work/target"

# run <name> <source> <expect: fail|pass> <diagnostic that a failure must carry>
run() {
    local name=$1 src=$2 expect=$3 diag=$4 dir="$work/$1" out
    mkdir -p "$dir/src"
    printf '[package]\nname = "%s"\nversion = "0.0.0"\nedition = "2024"\n' "$name" >"$dir/Cargo.toml"
    printf '%s\n' "$src" >"$dir/src/lib.rs"
    cp "$root/crates/sim/clippy.toml" "$dir/clippy.toml"
    cp "$root/rust-toolchain.toml" "$dir/rust-toolchain.toml"
    if out=$(cd "$dir" && cargo clippy --offline --all-targets -- -D warnings -W clippy::unwrap_used 2>&1); then
        [[ $expect == pass ]] && { echo "ok: $name passes clippy"; return 0; }
        echo "FAIL: $name passed clippy but must fail"
        return 1
    fi
    if [[ $expect == fail ]] && grep -q "$diag" <<<"$out"; then
        echo "ok: $name fails clippy with $diag"
        return 0
    fi
    echo "FAIL: $name: expected $expect ($diag); output:"
    echo "$out"
    return 1
}

status=0
run sin_in_code 'pub fn f(x: f64) -> f64 { x.sin() }' fail clippy::disallowed_methods || status=1
run fs_read_in_code 'pub fn f() -> Vec<u8> { std::fs::read("x").unwrap_or_default() }' fail clippy::disallowed_methods || status=1
run unwrap_in_test '#[cfg(test)]
mod tests {
    #[test]
    fn t() {
        std::hint::black_box(Some(1u8)).unwrap();
    }
}' pass - || status=1
exit "$status"
