#!/usr/bin/env bash
# Check formatting and lints. This is the gate that must stay green.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
