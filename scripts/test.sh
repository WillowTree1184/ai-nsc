#!/usr/bin/env bash
# Build (if needed) and run the full test suite.
#
#   scripts/test.sh [preset]      preset defaults to "dev"
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
preset="${1:-dev}"

cmake --preset "$preset" -S "$root"
cmake --build --preset "$preset"
ctest --preset "$preset" --output-on-failure
