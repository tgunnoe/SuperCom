#!/usr/bin/env python3
"""A tiny evaluator for the --run fixtures: report.py EXIT [STREAM TEXT...].

Prints TEXT to STREAM (stdout or stderr) when both are given, then exits with
status EXIT. It stands in for a real script that passes, fails, or skips.
"""

import sys


def main(argv: list[str]) -> int:
    status = int(argv[0])
    if len(argv) > 2:
        stream = sys.stdout if argv[1] == "stdout" else sys.stderr
        print(" ".join(argv[2:]), file=stream)
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
