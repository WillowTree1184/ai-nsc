# ai-nsc — the NovaSharp compiler

`ai-nsc` is the compiler for **NovaSharp**, a statically typed language that compiles to
native code through **LLVM 18**. The compiler is written in Rust and builds LLVM IR with
[inkwell](https://github.com/TheDan64/inkwell) (a safe Rust wrapper over LLVM's C++ IR API).
`clang` turns that IR into object files and executables.

## Status

The workspace is prepared and validated; the language front end is **not implemented yet**
because the NovaSharp grammar specification is still pending.

What already works:

- Cargo build against LLVM 18 (`llvm-18-dev`), linking the shared `libLLVM-18.so`
- LLVM pipeline proven end to end: inkwell module → textual IR → `clang` → real ELF object
- `ai-nsc` driver: `build`, `run`, `check`, `emit-ir`, `emit-obj`, `-o`, `-O0..-O3`, `-g`,
  `--target`, `--no-color`, `--version`, `--help`
- Source manager (buffers, byte spans, offset ↔ line/column) and a diagnostics renderer with
  `file:line:col: severity: message`, source excerpt and caret
- 25 tests: 16 unit, 7 CLI integration, 2 LLVM codegen integration
- `cargo fmt` + `clippy -D warnings` clean

See [docs/ROADMAP.md](docs/ROADMAP.md) for the plan and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design.

## Requirements

- Rust 1.75+ (developed with 1.98)
- LLVM 18 development files: `llvm-config` and `libLLVM-18.so` (Ubuntu: `llvm-18-dev`)
- `clang` 18 (object emission and linking)

`scripts/check-toolchain.sh` validates all three and lists optional tooling.

## Build and test

```bash
cargo build              # debug binary at target/debug/ai-nsc
cargo test               # unit + CLI + codegen tests
./scripts/lint.sh        # rustfmt --check and clippy -D warnings
cargo build --release    # optimized binary at target/release/ai-nsc
```

There is no CMake step; Cargo drives everything. If `llvm-config` is not LLVM 18 on your
`PATH`, point the build at the right installation:

```bash
LLVM_SYS_181_PREFIX=/usr/lib/llvm-18 cargo build
```

Shorthands: `scripts/build.sh [--release]`, `scripts/test.sh`, `scripts/format.sh`,
`scripts/lint.sh`.

## Layout

```
src/source.rs        FileId, SourceLocation, Span, SourceManager
src/diagnostics.rs   Severity, render(), DiagnosticEngine
src/driver.rs        Action, Options, argument parsing, dispatch
src/codegen.rs       LLVM IR generation (the only module that touches LLVM)
src/lib.rs           crate root and toolchain constants
src/main.rs          thin binary entry point
tests/               CLI and LLVM codegen integration tests
docs/                architecture, roadmap, toolchain notes
docs/grammar/        drop the NovaSharp grammar specification here
runtime/             NovaSharp runtime support library (planned)
examples/            example programs (added with the language)
scripts/             build, test, format, lint, toolchain checks
```

## Design notes

- The front end does not depend on LLVM; only `codegen` constructs IR.
- IR is built with inkwell (real LLVM API, not text templates) and verified before it is
  written out.
- `clang` is used as the assembler/linker driver, so the compiler does not embed a linker.
- Diagnostics never abort: phases collect errors, the driver decides the exit code.
- Rendering is a pure function, so diagnostic output is unit-tested byte for byte.
- `#![forbid(unsafe_code)]` at crate level.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and
[docs/TOOLCHAIN.md](docs/TOOLCHAIN.md).
