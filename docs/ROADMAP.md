# ai-nsc roadmap

Work is split into phases with concrete acceptance criteria. Phases are ordered;
later phases may start early only where they do not depend on the language spec.

## Phase 0 — workspace and toolchain (done)

- [x] CMake + Ninja presets (`dev`, `relwithdebinfo`, `release`, `asan`)
- [x] LLVM 18 integration (headers, shared lib, CMake package discovery)
- [x] `ai-nsc` driver skeleton with command line parsing and exit codes
- [x] SourceManager + DiagnosticEngine with caret excerpts
- [x] Unit-test harness, 12 tests, CTest wiring
- [x] Docs, scripts, formatting configuration
- [x] Verified end to end: LLVM module → verified IR → ELF object emitted

Acceptance: `ctest --preset dev` is green on a clean checkout.

## Phase 1 — grammar intake and AST design (next)

Depends on: the NovaSharp grammar specification.

- [ ] Formalize the grammar: token inventory, precedence/associativity table,
      statement and declaration forms, lexical corner cases (comments, literals,
      escapes, identifiers/keywords)
- [ ] Write `docs/GRAMMAR.md` as the single source of truth derived from the spec
- [ ] Design AST node set and produce `include/ainsc/Ast/`
- [ ] Design the type system and semantic model (primitives, references/values,
      nullability, user types, generics if any)
- [ ] Define the runtime ABI (how strings/arrays/collections are represented)
- [ ] Collect grammar example programs into `examples/` and `tests/e2e/cases/`

Acceptance: every construct in the spec maps to an AST node; the type-system and
runtime-ABI documents are reviewed before any parser code lands.

## Phase 2 — lexer and parser

- [ ] `Tokenizer` producing tokens with `SourceRange` spans
- [ ] Recursive-descent parser with precedence climbing for expressions
- [ ] Panic-mode error recovery at statement/declaration boundaries
- [ ] `--dump-tokens` and `--dump-ast`
- [ ] Diagnostics for unexpected tokens, unterminated literals/comments, etc.

Acceptance: every grammar example parses; AST dumps match reviewed golden dumps;
every parser error path has a malformed-input test.

## Phase 3 — semantic analysis

- [ ] Symbol tables and scopes, name resolution
- [ ] Type checking and inference (exactly as the spec allows)
- [ ] Flow checks the spec requires (definite assignment, unreachable code, ...)
- [ ] Constant folding needed for codegen (literal truth values, array sizes, ...)
- [ ] Lowering AST → typed HIR

Acceptance: a negative-test suite of ill-typed programs produces precise,
located diagnostics; all valid examples type-check.

## Phase 4 — IR generation

- [ ] Type mapping NovaSharp → LLVM types; value/reference representation
- [ ] Function codegen: locals, parameters, calls, returns
- [ ] Control flow: conditionals, loops, break/continue, short-circuit operators
- [ ] Aggregates: strings, arrays, collections as specified
- [ ] `llvm::verifyModule` on every module; `emit-ir` and `emit-obj` actions

Acceptance: IR golden tests with `FileCheck-18`; objects link and run for the
full example suite.

## Phase 5 — driver, linking and runtime

- [ ] `runtime/` builds `libnscrt` with the primitives generated code calls
- [ ] Driver: `build`, `run`, `check`, `emit-ir`, `emit-obj` fully wired,
      `-o`, `-O0..-O3`, `-g`, `--target`
- [ ] Link through the system toolchain; sensible defaults for output names
- [ ] Multi-file compilation units

Acceptance: `ai-nsc run examples/hello.nova` works from a clean build; `-O2` and
`-g` produce debug symbols that resolve.

## Phase 6 — hardening and tooling

- [ ] fuzz lexer/parser with libFuzzer or a mutation harness
- [ ] `asan` preset exercised in CI
- [ ] clang-tidy clean-up pass
- [ ] Error message polish and "did you mean" suggestions
- [ ] Benchmark happy-path compile time

Acceptance: no sanitizer findings on the test suite; clang-tidy has no
high-severity findings.

## Open questions (blocked on the spec)

1. Is NovaSharp value-based or reference-based for user types? This determines
   the memory model and the runtime ABI.
2. Does it have generics/templates? This decides monomorphization vs boxing.
3. Is there garbage collection, reference counting, or manual memory management?
4. Are strings immutable (C#-like) or mutable?
5. Is the runtime allowed to depend on libc/libstdc++?
6. Which file extension do NovaSharp sources use? (Currently `.nova` is a
   placeholder in the driver's help text.)
