#!/usr/bin/env bash
# Run the full test suite: unit tests plus CLI and codegen integration tests.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo test "$@"
