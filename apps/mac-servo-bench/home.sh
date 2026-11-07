#!/bin/zsh
set -eu
cd "$(dirname "$0")"
exec ./run.sh home "$@"
