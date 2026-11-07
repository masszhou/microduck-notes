#!/bin/zsh
set -eu
cd "$(dirname "$0")"
exec cargo run --quiet --locked -- "$@"
