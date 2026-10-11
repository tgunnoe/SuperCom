#!/bin/sh
# An executable evaluator that is not Python, for the --run fixtures: exits with its first argument.
exit "${1:-0}"
