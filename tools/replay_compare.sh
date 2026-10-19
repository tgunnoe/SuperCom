#!/usr/bin/env bash
# Compare the digest program's output across two runs (DET-024).
#
#   tools/replay_compare.sh --local
#   tools/replay_compare.sh <user@host> [remote-checkout-dir]
#
# --local runs the program twice here as two processes. A host argument runs
# it here and over ssh in a checkout of the same commit on the remote, and
# compares the two outputs; REMOTE_NIX_SHELL names the remote's shell file
# (default: the checkout's shell.nix) and REMOTE_FA_DIR the remote install
# (default: this host's SUPERCOM_FA_DIR). The tree is never copied: the same
# commit must be checked out on both sides. Exits 0 on EQUAL, 1 on a
# difference, 2 on a usage or precondition failure.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

program=(cargo run -q --release -p supercom-replay-harness --bin replay-digests)

die() {
    echo "replay_compare: $*" >&2
    exit 2
}

# compare <a> <b>: EQUAL with the line count, or the first differing lines.
compare() {
    if cmp -s "$1" "$2"; then
        echo "EQUAL $(wc -l <"$1") lines"
        return 0
    fi
    echo "DIFFERENT: first differing lines (diff, three lines of context):"
    diff -U3 "$1" "$2" | head -n 12 || true
    return 1
}

[[ $# -ge 1 ]] || die "usage: $0 --local | <user@host> [remote-checkout-dir]"

if [[ $1 == --local ]]; then
    cd "$root"
    "${program[@]}" >"$work/a.txt" 2>"$work/a.err"
    "${program[@]}" >"$work/b.txt" 2>"$work/b.err"
    compare "$work/a.txt" "$work/b.txt"
    exit $?
fi

host=$1
remote_dir=${2:-supcom}
local_fa=${SUPERCOM_FA_DIR:-}
remote_fa=${REMOTE_FA_DIR:-$local_fa}
[[ -n $remote_fa ]] || die "set REMOTE_FA_DIR or SUPERCOM_FA_DIR"
remote_shell=${REMOTE_NIX_SHELL:-shell.nix}

local_head=$(git -C "$root" rev-parse HEAD)
remote_head=$(ssh "$host" "git -C $remote_dir rev-parse HEAD")
echo "local : $(uname -m) $local_head"
echo "remote: $(ssh "$host" uname -m) $remote_head"
[[ $local_head == "$remote_head" ]] || die "the two checkouts differ"

cd "$root"
"${program[@]}" >"$work/local.txt" 2>"$work/local.err"
ssh "$host" "cd $remote_dir && SUPERCOM_FA_DIR=$remote_fa nix-shell $remote_shell --run '${program[*]}'" \
    >"$work/remote.txt" 2>"$work/remote.err"
echo "local stderr:"
cat "$work/local.err"
echo "remote stderr:"
cat "$work/remote.err"
compare "$work/local.txt" "$work/remote.txt"
