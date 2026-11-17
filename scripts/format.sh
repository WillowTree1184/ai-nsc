#!/usr/bin/env bash
# Format all C++ sources in place.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

formatter="$(command -v clang-format-18 || command -v clang-format || true)"
if [[ -z "$formatter" ]]; then
  echo "error: clang-format not found (install clang-format-18)" >&2
  exit 1
fi

mapfile -t files < <(
  find "$root/src" "$root/include" "$root/tests" \
    -type f \( -name '*.h' -o -name '*.cpp' \) | sort
)

if [[ ${#files[@]} -eq 0 ]]; then
  echo "no sources found"
  exit 0
fi

"$formatter" -i "${files[@]}"
echo "formatted ${#files[@]} file(s) with ${formatter}"
