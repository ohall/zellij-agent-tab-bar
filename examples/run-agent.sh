#!/bin/sh
set -eu

if [ "$#" -eq 0 ]; then
    printf 'usage: %s <command> [args...]\n' "$0" >&2
    exit 2
fi

exec zja run -- "$@"
