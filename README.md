# ai-nsc — the NovaSharp compiler

`ai-nsc` is the compiler for **NovaSharp**, a statically typed programming language
that targets native code through **LLVM 18**. The compiler itself is written in
C++20: a hand-written lexer and recursive-descent parser, a dedicated
semantic-analysis pass, and LLVM's C++ API for code generation. The driver,
linker invocation and runtime support library are part of this repository.

## Status

The workspace is prepared and validated; the language front end is **not
implemented yet** because the NovaSharp grammar specification is still pending.

What already works:

- CMake/Ninja build with LLVM 18 (shared `libLLVM-18.so`), `-Wall -Wextra -Wpedantic`
- `ai-nsc` driver skeleton: `--version`, `--help`, `build`, `run`, `check`,
  `emit-ir`, `emit-obj`, `-o`, `-O0..-O3`, `-g`, `--target`, `--no-color`
- Source manager (file buffers, offset ↔ line/column mapping) and a diagnostic
  engine that prints `file:line:col: severity: message` plus source excerpt and caret
- Unit-test harness and tests (including an LLVM smoke test that emits a real ELF
  object through the target machine)
- Docs, presets and scripts

See [docs/ROADMAP.md](docs/ROADMAP.md) for the plan and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design.

## Requirements

- Linux x86-64 (primary host; codegen can target other LLVM targets)
- CMake ≥ 3.20, Ninja
- Clang 18 or GCC 13+ with C++20 support
- LLVM 18 development files: `llvm-18-dev` (headers + shared `libLLVM-18.so`)

`scripts/check-toolchain.sh` validates the environment and lists optional
tooling that is missing.

## Build and test

```bash
cmake --preset dev
cmake --build --preset dev
ctest --preset dev --output-on-failure
```

Presets: `dev` (Debug), `relwithdebinfo`, `release`, `asan` (ASan + UBSan).
Artifacts land in `out/build/<preset>/`: `ai-nsc` and `tests/ainsc-unit-tests`.

```bash
out/build/dev/ai-nsc --version
out/build/dev/ai-nsc --help
```

Shorthands: `scripts/build.sh [preset]`, `scripts/test.sh [preset]`,
`scripts/format.sh` (needs `clang-format-18`).

## Layout

```
include/ainsc/Base/     source locations, source manager, diagnostics
include/ainsc/Driver/   command line driver
src/                    implementations of the above
tests/unit/             in-tree unit-test harness and unit tests
tests/e2e/              end-to-end compiler tests (planned)
docs/                   architecture, roadmap, toolchain notes
docs/grammar/           drop the NovaSharp grammar specification here
runtime/                NovaSharp runtime support library (planned)
scripts/                build, test, format and toolchain checks
```

## Design notes

- The front end does not depend on LLVM IR; only the codegen layer touches
  `llvm::IRBuilder`.
- Diagnostics never abort: phases collect errors, the driver decides the exit code.
- Generated IR is run through `llvm::verifyModule` before it reaches the back end.
- The compiler links the shared `libLLVM-18.so` for fast incremental builds.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and
[docs/TOOLCHAIN.md](docs/TOOLCHAIN.md).
