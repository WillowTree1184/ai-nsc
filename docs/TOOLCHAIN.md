# Toolchain

## Host

| Item | Value |
|---|---|
| OS | Ubuntu 24.04.5 LTS (noble), x86-64 |
| CPUs / RAM | 8 / 15 GiB |
| CMake | 3.28.3 |
| Ninja | 1.11.1 |
| Clang / Clang++ | 18.1.3 |
| GCC / G++ | 13.3.0 |
| LLVM (dev) | 18.1.3 — headers, static component libs and shared `libLLVM-18.so` |
| Python | 3.12 |

LLVM 18 is the project baseline. Other LLVM majors (17, 20, 21) are present on
the machine for other projects but `llvm-18-dev` is the only one with headers
installed; `CMakeLists.txt` points `AINSC_LLVM_DIR` at
`/usr/lib/llvm-18/lib/cmake/llvm`.

To switch LLVM versions later, pass `-DAINSC_LLVM_DIR=/usr/lib/llvm-<N>/lib/cmake/llvm`
(any LLVM with headers and a CMake package works; the code targets the LLVM 18
C++ API and would need small adjustments for others).

## Already installed (required)

`cmake`, `ninja-build`, `clang-18`/`g++-13`, `llvm-18-dev` (headers +
`libLLVM-18.so`), `llvm-18-tools` (`FileCheck-18`, `llc`, `opt`, `llvm-as`,
`lli`), `lld`, `gdb`, `lldb`, `git`, `python3`.

Verify with:

```bash
./scripts/check-toolchain.sh
```

## Recommended apt packages (not required to build)

These improve the day-to-day loop but the build and tests pass without them:

```bash
sudo apt-get update
sudo apt-get install -y \
  ccache \
  clang-format-18 \
  clang-tidy-18 \
  clangd-18 \
  valgrind \
  doxygen \
  libgtest-dev
```

| Package | Why |
|---|---|
| `ccache` | fast rebuilds of the C++ front end; enable with `-DCMAKE_CXX_COMPILER_LAUNCHER=ccache` |
| `clang-format-18` | `scripts/format.sh`; style is checked in `.clang-format` |
| `clang-tidy-18` | static analysis driven by `.clang-tidy` |
| `clangd-18` | editor/IDE support using `compile_commands.json` |
| `valgrind` | memory debugging alternative to the `asan` preset |
| `doxygen` | API documentation generation (planned) |
| `libgtest-dev` | only if unit tests migrate from the in-tree harness to GoogleTest |

`graphviz` (already installed) is available for Doxygen graphs.

Note: the agent shell runs with `no_new_privs`, so `sudo` cannot be used from
inside it; install packages from a normal terminal.

## Build

```bash
cmake --preset dev                 # configure
cmake --build --preset dev         # build
ctest --preset dev --output-on-failure
```

Artifacts:

- `out/build/dev/ai-nsc`
- `out/build/dev/tests/ainsc-unit-tests`
- `out/build/dev/compile_commands.json`

## Editor setup

Point clangd at the build directory produced by the preset; it discovers
`compile_commands.json` there automatically. Optionally create `.clangd`:

```yaml
CompileFlags:
  CompilationDatabase: out/build/dev
```

## Troubleshooting

- **`LLVMConfig.cmake` not found** — install `llvm-18-dev`, or pass
  `-DAINSC_LLVM_DIR=<dir containing LLVMConfig.cmake>`.
- **`llvm/Support/Host.h` not found** — LLVM 18 moved it to
  `llvm/TargetParser/Host.h`; the code already uses the new location.
- **Static-link candidate errors** (`libedit`, `libcurl`, `libxml2`) during
  configure are harmless: the build links the shared `libLLVM-18.so`, so no
  LLVM-internal dependencies are required.
- **Link takes seconds** — expected when linking the shared LLVM library; use
  ccache and Ninja to keep it fast.
