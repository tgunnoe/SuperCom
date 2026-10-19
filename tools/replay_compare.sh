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
# commit must be checked out on both sides. The directory, the install, and
# the shell file are taken literally: a leading ~ or a $ is not expanded on
# the remote. Exits 0 on EQUAL, 1 on a difference, 2 on a usage or
# precondition failure, including a failed program run, whose captured
# standard error is printed.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

program=(cargo run -q --release -p supercom-replay-harness --bin replay-digests)

die() {
    echo "replay_compare: $*" >&2
    exit 2
}

# failed <label> <errfile>: report a failed run's captured stderr and exit 2.
failed() {
    echo "replay_compare: $1 run failed; its stderr:" >&2
    cat "$2" >&2
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
    "${program[@]}" >"$work/a.txt" 2>"$work/a.err" || failed local "$work/a.err"
    "${program[@]}" >"$work/b.txt" 2>"$work/b.err" || failed local "$work/b.err"
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
q_dir=$(printf %q "$remote_dir")
remote_head=$(ssh "$host" "git -C $q_dir rev-parse HEAD")
echo "local : $(uname -m) $local_head"
echo "remote: $(ssh "$host" uname -m) $remote_head"
[[ $local_head == "$remote_head" ]] || die "the two checkouts differ"

cd "$root"
"${program[@]}" >"$work/local.txt" 2>"$work/local.err" || failed local "$work/local.err"
# Each value is quoted for the remote shell; the program's words are quoted
# once as a command and again as the one argument of --run.
q_fa=$(printf %q "$remote_fa")
q_shell=$(printf %q "$remote_shell")
q_run=$(printf %q "$(printf '%q ' "${program[@]}")")
ssh "$host" "cd $q_dir && SUPERCOM_FA_DIR=$q_fa nix-shell $q_shell --run $q_run" \
    >"$work/remote.txt" 2>"$work/remote.err" || failed remote "$work/remote.err"
echo "local stderr:"
cat "$work/local.err"
echo "remote stderr:"
cat "$work/remote.err"
compare "$work/local.txt" "$work/remote.txt"
