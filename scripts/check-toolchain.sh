#!/usr/bin/env bash
# Validate the environment ai-nsc needs. Never installs anything.
set -uo pipefail

status=0

report() {
  printf '  %-26s %s\n' "$1" "$2"
}

echo "== required =="

if command -v rustc >/dev/null 2>&1; then
  report "rustc" "$(rustc --version)"
else
  report "rustc" "MISSING -> https://rustup.rs"
  status=1
fi

if command -v cargo >/dev/null 2>&1; then
  report "cargo" "$(cargo --version)"
else
  report "cargo" "MISSING -> https://rustup.rs"
  status=1
fi

llvm_config="$(command -v llvm-config || true)"
if [[ -n "$llvm_config" ]]; then
  llvm_version="$("$llvm_config" --version)"
  report "llvm-config" "$llvm_version ($llvm_config)"
  if [[ "$llvm_version" != 18.* ]]; then
    report "LLVM version" "expected 18.x, found $llvm_version"
  fi
  libdir="$("$llvm_config" --libdir)"
  if compgen -G "$libdir/libLLVM-18.so*" >/dev/null; then
    report "libLLVM-18.so" "$libdir"
  else
    report "libLLVM-18.so" "MISSING -> apt install llvm-18-dev"
    status=1
  fi
else
  report "llvm-config" "MISSING -> apt install llvm-18-dev"
  status=1
fi

if command -v clang >/dev/null 2>&1; then
  report "clang" "$(clang --version | head -n1)"
else
  report "clang" "MISSING -> apt install clang-18"
  status=1
fi

echo
echo "== optional =="
rustup_missing=()
apt_missing=()

if command -v rustfmt >/dev/null 2>&1; then
  report "rustfmt" "installed"
else
  report "rustfmt" "MISSING -> rustup component add rustfmt"
  rustup_missing+=("rustfmt")
fi

if command -v cargo-clippy >/dev/null 2>&1; then
  report "clippy" "installed"
else
  report "clippy" "MISSING -> rustup component add clippy"
  rustup_missing+=("clippy")
fi

if command -v rust-analyzer >/dev/null 2>&1; then
  report "rust-analyzer" "installed"
else
  report "rust-analyzer" "MISSING -> rustup component add rust-analyzer"
  rustup_missing+=("rust-analyzer")
fi

check_apt_tool() {
  local label="$1" command="$2" package="$3"
  if command -v "$command" >/dev/null 2>&1; then
    report "$label" "installed"
  else
    report "$label" "MISSING ($package)"
    apt_missing+=("$package")
  fi
}

check_apt_tool "valgrind" valgrind valgrind
check_apt_tool "FileCheck" FileCheck-18 llvm-18-tools
check_apt_tool "lld" ld.lld lld

echo
if [[ ${#rustup_missing[@]} -gt 0 ]]; then
  echo "Install Rust components with:"
  echo "  rustup component add ${rustup_missing[*]}"
fi
if [[ ${#apt_missing[@]} -gt 0 ]]; then
  echo "Install system packages with:"
  echo "  sudo apt-get install -y ${apt_missing[*]}"
fi
if [[ ${#rustup_missing[@]} -eq 0 && ${#apt_missing[@]} -eq 0 ]]; then
  echo "All optional tooling is installed."
fi

exit "$status"
