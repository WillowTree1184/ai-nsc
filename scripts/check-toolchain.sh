#!/usr/bin/env bash
# Validate the environment ai-nsc needs and list recommended packages that are
# missing. Safe to run at any time; never installs anything itself.
set -uo pipefail

status=0

report() {
  printf '  %-26s %s\n' "$1" "$2"
}

require_command() {
  local label="$1" command="$2"
  if command -v "$command" >/dev/null 2>&1; then
    report "$label" "$(command -v "$command")"
  else
    report "$label" "MISSING"
    status=1
  fi
}

require_versioned() {
  local label="$1" command="$2"
  if command -v "$command" >/dev/null 2>&1; then
    report "$label" "$("$command" --version 2>/dev/null | head -n1)"
  else
    report "$label" "MISSING"
    status=1
  fi
}

echo "== required =="
require_versioned "CMake" cmake
require_versioned "Ninja" ninja
if command -v clang++ >/dev/null 2>&1; then
  report "C++ compiler" "$(clang++ --version | head -n1)"
elif command -v g++ >/dev/null 2>&1; then
  report "C++ compiler" "$(g++ --version | head -n1)"
else
  report "C++ compiler" "MISSING (clang++ or g++)"
  status=1
fi

llvm_config="/usr/lib/llvm-18/bin/llvm-config"
if [[ -x "$llvm_config" ]]; then
  report "LLVM 18 (config)" "$("$llvm_config" --version)"
  if [[ -f /usr/lib/llvm-18/include/llvm/IR/Module.h ]]; then
    report "LLVM 18 (headers)" "/usr/lib/llvm-18/include"
  else
    report "LLVM 18 (headers)" "MISSING -> apt install llvm-18-dev"
    status=1
  fi
  if [[ -f /usr/lib/llvm-18/lib/libLLVM-18.so ]]; then
    report "LLVM 18 (shared lib)" "/usr/lib/llvm-18/lib/libLLVM-18.so"
  else
    report "LLVM 18 (shared lib)" "MISSING -> apt install llvm-18-dev"
    status=1
  fi
else
  report "LLVM 18 (config)" "MISSING -> apt install llvm-18-dev"
  status=1
fi

require_command "FileCheck" FileCheck-18
require_command "lld" ld.lld

echo
echo "== optional =="
optional_missing=()
check_optional_command() {
  local label="$1" command="$2" package="$3"
  if command -v "$command" >/dev/null 2>&1; then
    report "$label" "installed"
  else
    report "$label" "MISSING ($package)"
    optional_missing+=("$package")
  fi
}

check_optional_command "ccache" ccache ccache
check_optional_command "clang-format" clang-format-18 clang-format-18
check_optional_command "clang-tidy" clang-tidy-18 clang-tidy-18
check_optional_command "clangd" clangd-18 clangd-18
check_optional_command "valgrind" valgrind valgrind
check_optional_command "doxygen" doxygen doxygen
if [[ -f /usr/include/gtest/gtest.h ]]; then
  report "GoogleTest headers" "installed"
else
  report "GoogleTest headers" "MISSING (libgtest-dev)"
  optional_missing+=("libgtest-dev")
fi

echo
if [[ ${#optional_missing[@]} -gt 0 ]]; then
  echo "Recommended packages not installed:"
  printf '  %s\n' "${optional_missing[@]}"
  echo
  echo "Install them with:"
  echo "  sudo apt-get update && sudo apt-get install -y ${optional_missing[*]}"
else
  echo "All optional tooling is installed."
fi

exit "$status"
