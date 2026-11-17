#!/usr/bin/env bash
# Build ai-nsc.
#
#   scripts/build.sh            debug build
#   scripts/build.sh --release  optimized build
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo build "$@"
