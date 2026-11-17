# ai-nsc — the NovaSharp compiler

`ai-nsc` is the compiler for **NovaSharp**, a statically typed language that compiles to
native code through **LLVM 18**. The compiler is written in Rust and builds LLVM IR with
[inkwell](https://github.com/TheDan64/inkwell) (a safe Rust wrapper over LLVM's C++ IR API).
`clang` turns that IR into object files and executables.

## Status

The compiler now runs end to end for the implemented subset: lexer → parser →
semantic analysis → LLVM IR (inkwell) → `clang` → native executable.
`ai-nsc run program.ns` executes the program and returns `Main`'s value as the
process exit code (§14.2).

Implemented today:

- **Lexer** for the complete lexical grammar (keywords with `@` escape, all operators,
  integer/float/char/string/verbatim literals with encoding tags, comments)
- **Literal decoder**: bases, `_` separators, escapes, ascii/utf8/utf16/utf32/ucs2/ucs4,
  multi-line string rules
- **AST and parser** covering the grammar, validated against every program example in the
  spec (89/90; one is a bare type fragment)
- **Semantic analysis**: scopes and shadowing, named types, structure layout (spec alignment
  rules), enumeration values, name resolution (locals, globals, fields, enum members,
  modules), overload resolution by argument types, literal typing from context, the
  operator type rules, array/pointer/string literal types, negative diagnostics
- **LLVM code generation**: functions and recursion, locals and globals, integer/cardinal/
  float/boolean arithmetic with wrapping and signedness-correct division, comparisons,
  short-circuit logical operators, branches and loops, `match`, calls, fixed arrays and
  strings, enumerations, structures (value layout and field access), numeric casts, and the
  generated `main` launcher
- **Object model**: extension blocks with methods, `this` parameters and method calls,
  structure/class construction (`T(...)`, `Construct` overloads, default construction, field
  initializers), class instance layout (vtable/typeinfo header + fields, single inheritance
  field flattening), class reference semantics, `heap` and `dispose` (destructor + free)
- **Polymorphism**: `virtual`/`override` with vtables, `sealed` classes/methods, `base.M()`
  non-dispatched base calls, and RTTI-backed `is` checks over class hierarchies
- **Variants and unions**: state/payload layout (smallest usable tag, aligned payload space),
  union overlap layout with `aligned(N)`, member construction (`Variant.State(value)`,
  `Union.Member(value)`), payload-less variant states, union type punning, `is` checks for
  variant states, and `match` `is` patterns with guards and payload binding
- **Interfaces**: contract declarations, implementation checking on classes (including
  interfaces inherited through base classes), implicit class→interface conversion,
  per-class interface dispatch tables with virtual interface calls, and `is` checks
  against interface types through RTTI
- **Accessors**: `accessor` properties in extension blocks and interface bodies with
  `=> backing` default get/set, explicit `get`/`set` expression and block forms, getter/setter
  methods (mangled `Name@get` / `Name@set`), read-only diagnostics and interface accessor
  dispatch through interface tables
- **Operator overloading**: binary `+ - * / % == > <`, unary `-`, indexers with `get`/`set`
  accessor members, and conversion operators; derived `!=` / `<=` / `>=` desugared from `==`
  and `<`/`>`; user overloads take precedence over built-ins during check and codegen
- **Exceptions**: `trap`/`catch`/`finally` with lexical matching, `throw` in trap and catch
  bodies, type matching by exact type, class base chain and interface tables, catch bindings,
  ordered/wildcard clauses, propagation to enclosing traps and program termination;
  runtime exception sources raise the Built-In Library exception classes: division by zero
  raises `DivideByZeroException`, array index out of bounds raises `OutOfRangeException`, and
  they can be caught by their subclasses or by `Exception`
- **Compile-time conditionals**: `#if`/`#else` evaluated at compile time over boolean
  constants and constant comparisons; only the selected branch is checked and generated
- **Name mangling** per §13.2, including the spec's worked example as a unit test
- 99 tests: 36 unit, 8 CLI, 2 LLVM smoke, 42 end-to-end execution, 10 parser, 1 spec
  conformance; `cargo fmt` + `clippy -D warnings` clean

Not implemented yet (rejected with a clear diagnostic): generics/monomorphization,
function literals and closures, runtime exception sources and the Built-In Library exception
hierarchy, compile-time execution (`#(...)`, `#{...}`, `#if`), references (`&`/`&T`) beyond
`this`, the `??` operator and `implicitable` (implicit conversion) invocations, NOLAA, and the
Built-In Library prelude.

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
docs/grammar/        the language specification (Draft 1.12 is current)
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
