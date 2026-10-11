#!/usr/bin/env python3
"""Evaluator for the repository-reading clauses of docs/specs/data-model.md (DAT-022, DAT-026).

Both clauses describe code, so the script reads the repository only: it needs no install
and runs without one. Prints constants and counts only.
"""

import os
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "formats"))
import fmtlib as F

IMPORTER = "crates/bp-import/src/lib.rs"
BENCH = "crates/bp-import/src/bin/bench_import.rs"


def key_blindness(_game) -> None:
    source = F.repo_text(IMPORTER)
    body = re.search(r"pub fn cache_key\(.*?\n}\n", source, re.S)
    if body is None:
        raise F.Fail(f"{IMPORTER} has no cache_key function")
    text = body.group(0)
    F.expect("key_reads_size", "meta.len()" in text, True)
    F.expect("key_reads_mtime", "modified" in text, True)
    F.expect("key_leads_with_format", 'format!("v{CACHE_FORMAT}")' in text, True)
    F.expect("format_is_manual_const", re.search(r"const CACHE_FORMAT: u32 = \d+;", source) is not None, True)
    digest_words = [w for w in ("sha", "digest", "blake", "crc", "hash", "include_str") if w in source.lower()]
    F.expect("content_digest_words", digest_words, [])


def budget(_game) -> None:
    source = F.repo_text(BENCH)
    cold = re.search(r"const COLD_BUDGET: Duration = Duration::from_secs\((\d+)\);", source)
    cached = re.search(r"const CACHED_BUDGET: Duration = Duration::from_secs\((\d+)\);", source)
    if cold is None or cached is None:
        raise F.Fail(f"{BENCH} lacks COLD_BUDGET or CACHED_BUDGET")
    F.expect("cold_budget_seconds", int(cold.group(1)), 10)
    F.expect("cached_budget_seconds", int(cached.group(1)), 1)
    printed = re.findall(r'println!\("(\w+) ', source)
    F.expect("printed_lines", printed, ["cold_import", "cache_write", "cached_load", "mount_share"])


CHECKS = {"DAT-022": key_blindness, "DAT-026": budget}

if __name__ == "__main__":
    sys.exit(F.S.run(CHECKS, sys.argv[1:], offline=frozenset(CHECKS)))
