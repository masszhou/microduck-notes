#!/bin/bash
set -euo pipefail
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
exec python3 "$(cd -- "$(dirname -- "$0")" && pwd)/watch-camera.py" "$@"
