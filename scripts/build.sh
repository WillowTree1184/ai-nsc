#!/usr/bin/env bash
# Configure and build ai-nsc.
#
#   scripts/build.sh [preset]     preset defaults to "dev"
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
preset="${1:-dev}"

cmake --preset "$preset" -S "$root"
cmake --build --preset "$preset"
