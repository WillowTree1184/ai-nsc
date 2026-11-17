# ai-nsc roadmap

Work is split into phases with concrete acceptance criteria. Phases are ordered; later
phases may start early only where they do not depend on the language spec.

## Phase 0 — workspace and toolchain (done)

- [x] Rust crate (`ai-nsc` binary + `ainsc` library), `rust-toolchain.toml`
- [x] inkwell 0.11 against LLVM 18.1.3, dynamic link to `libLLVM-18.so`
- [x] Driver skeleton: commands, options, exit codes
- [x] `SourceManager` (buffers, spans, line/column) and pure diagnostic renderer
- [x] 25 tests green (unit, CLI, LLVM codegen), `cargo fmt` + `clippy -D warnings` clean
- [x] Docs, scripts, `#![forbid(unsafe_code)]`
- [x] Verified end to end: inkwell IR → `clang` → ELF object

Acceptance: `cargo test` and `scripts/lint.sh` are green on a clean checkout.

## Phase 1 — grammar intake and AST design (in progress)

Depends on: the NovaSharp grammar specification (Draft 1.12 available).

- [x] Exhaustive per-section extraction of the spec into `docs/spec-notes/` (all EBNF blocks
      verbatim, line-anchored semantic rules, open questions)
- [x] Token inventory, precedence table and lexical corner cases (implemented in the lexer)
- [x] Design the AST node set and the arena layout (`src/ast.rs`)
- [ ] Write `docs/GRAMMAR.md` as the single source of truth derived from the spec
- [ ] Design the type system and semantic model (primitives, value/reference semantics,
      nullability, user types, generics if any)
- [ ] Define the runtime ABI (string/array/collection representations)
- [ ] Collect grammar examples into `examples/` and `tests/e2e/cases/`

Acceptance: every construct in the spec maps to an AST node; type-system and runtime-ABI
documents are reviewed before parser code lands.

## Phase 2 — lexer and parser (in progress)

- [x] `Lexer` producing tokens with `Span`s (`src/lexer.rs`), literal decoding (`src/literal.rs`)
- [x] Recursive-descent parser with precedence climbing (`src/parser/`)
- [x] Panic-mode recovery at statement/declaration boundaries
- [x] Conformance test: all 89 program examples in the spec parse
- [x] `--dump-tokens` and `--dump-ast` wired into the driver
- [ ] Diagnostics for unexpected tokens, unterminated literals/comments, etc. (lexer side done)
- [ ] Snapshot tests for AST dumps and error messages

Acceptance: every grammar example parses; AST dumps match reviewed snapshots; every parser
error path has a malformed-input test.

## Phase 3 — semantic analysis

- [ ] Symbol tables and scopes, name resolution
- [ ] Type checking and inference exactly as the spec allows
- [ ] Flow checks the spec requires (definite assignment, unreachable code, ...)
- [ ] Constant folding needed for codegen
- [ ] Lowering AST → typed HIR (`src/sema/`)

Acceptance: a negative-test suite of ill-typed programs produces precise, located
diagnostics; all valid examples type-check.

## Phase 4 — IR generation (in progress)

- [x] NovaSharp → LLVM type mapping; mangled symbols per §13.2; generated `main` launcher
- [x] Function codegen: locals, parameters, calls, returns (including recursion)
- [x] Control flow: conditionals, loops, break/continue, short-circuit operators, `match`
- [x] Aggregates: strings and fixed arrays, enumerations, structures (value layout)
- [x] `emit-ir`, `emit-obj`, `build` and `run` actions wired through `clang`
- [x] `module.verify()` on every generated module
- [x] Construction expressions (in-place, constructor overloads, field initializers)
- [x] Methods, `this`, class instances and `heap`/`dispose` allocation
- [ ] `??` operator and `implicitable` implicit conversions
- [ ] Runtime exception model (`trap`/`throw`/`finally`, division/out-of-range traps)
- [ ] NOLAA analysis for heap lifetime

Acceptance: IR golden tests with `FileCheck-18`; objects link and run for the full example
suite.

## Phase 5 — driver, linking and runtime

- [ ] `runtime/` compiled to `libnscrt.a` (C, built via `build.rs` with `clang`)
- [ ] `build`, `run`, `check` fully wired; `-o`, `-O0..-O3`, `-g`, `--target`
- [ ] Link through `clang`; sensible defaults for output names
- [ ] Multi-file compilation units

Acceptance: `ai-nsc run examples/hello.nova` works from a clean build; `-O2` and `-g`
produce the expected results.

## Phase 6 — hardening and tooling

- [ ] `cargo-fuzz` targets for lexer and parser
- [ ] clippy pedantic pass and error-message polish
- [ ] "did you mean ...?" suggestions
- [ ] Benchmark happy-path compile time

Acceptance: no fuzzer findings on the corpus; clean lints; measured baseline compile time.

## Open questions (blocked on the spec)

1. Is NovaSharp value-based or reference-based for user types? This determines the memory
   model and the runtime ABI.
2. Does it have generics/templates? This decides monomorphization vs boxing.
3. Is there garbage collection, reference counting, or manual memory management?
4. Are strings immutable (C#-like) or mutable?
5. Is the runtime allowed to depend on libc/libstdc++?
6. Which file extension do NovaSharp sources use? (`.nova` is a placeholder in the driver's
   help text.)
