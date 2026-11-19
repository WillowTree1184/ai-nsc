# ai-nsc — the NovaSharp compiler

`ai-nsc` is the compiler for **NovaSharp**, a statically typed language that compiles to
native code through **LLVM 18**. The compiler is written in Rust and builds LLVM IR with
[inkwell](https://github.com/TheDan64/inkwell) (a safe Rust wrapper over LLVM's C++ IR API).
`clang` turns that IR into object files and executables.

## Status

The compiler runs end to end: lexer → parser → semantic analysis → LLVM IR
(inkwell) → `clang` → native executable. `ai-nsc run program.ns` executes the
program and returns `Main`'s value as the process exit code (§14.2); several
input files form one program (§11.2).

Implemented and tested today:

- **Front end**: complete lexical grammar (including `@` forced keywords, `@` no-optimize markers, verbatim
  strings and encoding tags), literal decoder, recursive-descent parser validated
  against every code example in the specification, and a negative-diagnostics
  semantic analysis (scopes, modules, `use`, `owner`, shadowing, accessibility,
  `readonly`, `scoped`, `aligned`, `implicitable`, `static`).
- **Types and layout**: basic types, fixed arrays and variable-length arrays,
  pointers, function types with calling conventions, structures, classes with
  vtable/RTTI/interface tables, enumerations, variants and unions, exact layout
  and alignment rules, generic declarations with constraints and monomorphization.
- **Expressions and statements**: arithmetic/floats with wrapping and signedness
  rules, `?:`, `??` with `Optional`/`Result`, casts and `implicitable`,
  references (`&`/`*`), all operators with overloads and derived comparisons,
  accessors and indexers, loops (including `for` over arrays/VLAs/ranges/
  `IIterator`/`IIterable`), `match` with patterns/guards/`is` special floats,
  `trap`/`catch`/`finally`, `throw`, `heap`/`dispose`, compile-time blocks and
  conditionals, and the compiler API (`Type.SizeOf`, `Compiler.Log*`,
  `Compiler.Assert`, `Environment.Current/Host/Target` queries, `NOLAA.*`).
- **Functions and runtime**: defaults, named arguments, by-reference parameters,
  array parameter sugar, `#` compile-time constant parameters (functions,
  methods and operators), nested functions, function literals with capturing
  closures, function pointers, virtual/interface dispatch, exceptions with class
  and variant matching, custom `IAllocator`/`DefaultAllocator` heap allocation,
  NOLAA dispose warnings and conservative auto-dispose, and a launcher that
  performs runtime initialization before `Main`.
- **Built-In Library (§15)**: `Object`, `CallingConvention`, `PlatformKind`, `ArchitectureKind`, `EnvironmentKind`, the
  exception hierarchy with `InnerException` and constructors, `Optional<T>`,
  `Result<TValue; TAlternative>`, `IIterator<T>`, `IIterable<T>`, `IAllocator`
  and the platform `DefaultAllocator`, plus the compile-time library.

`cargo test` runs **402 tests** (41 lib unit, 134 CLI, 2 LLVM codegen, 209
end-to-end programs, 10 parser, 3 spec-conformance tests that read the
specification itself: every `novasharp` example parses, and every whole-program
example that passes semantic analysis is compiled end to end by the CLI).
`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean.
`docs/COVERAGE.md` maps every specification section to its status and evidence,
and `docs/AUDIT.md` records the 60 reproduced defects plus the commit that fixed
each one.

Known deliberate limitations (all diagnosed rather than silently miscompiled):
compile-time blocks are lowered as runtime equivalents of their compile-time
results; multi-dimensional VLAs, generic methods with their own type parameters,
array extension targets and some `scoped`/context-keyword nuances are reported as not
implemented; NOLAA analysis is a conservative subset; monomorphization-volume
optimization (§14.3.2) is not implemented; code generation targets the host ABI
and diagnoses calling conventions the host backend cannot represent. The
specification is a draft; behavior is kept adaptable (spec evidence lives in
`docs/spec-notes/` and the conformance tests).

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
src/lexer.rs         TokenKind, Token, hand-written lexer
src/literal.rs       literal decoding (bases, escapes, encodings)
src/ast.rs           arena AST for the whole language
src/parser/          recursive-descent parser (core + expressions)
src/sema/            analysis: types, scopes, layouts, resolution, checking
src/codegen.rs       inkwell IR generation + launcher
src/diagnostics.rs   Severity, render(), DiagnosticEngine
src/driver.rs        Action, Options, argument parsing, dispatch
src/codegen.rs       LLVM IR generation (the only module that touches LLVM)
src/lib.rs           crate root and toolchain constants
src/main.rs          thin binary entry point
tests/               CLI and LLVM codegen integration tests
docs/                architecture, roadmap, toolchain notes
docs/grammar/        the language specification (Draft 1.153 is current)
docs/spec-notes/     exhaustive per-section extraction of the spec + open questions
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
