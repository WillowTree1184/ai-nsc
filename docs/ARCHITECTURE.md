# ai-nsc architecture

This document describes the intended structure of the NovaSharp compiler. The crate in
this repository is the foundation described under "Current state".

## Current state

```
src/source.rs        FileId, SourceLocation, Span, SourceManager
src/diagnostics.rs   Severity, render(), DiagnosticEngine
src/driver.rs        Action, Options, parse_args(), run()
src/codegen.rs       LLVM IR generation (inkwell smoke module for now)
src/lib.rs           crate root, toolchain constants
src/main.rs          binary entry point
```

Everything else is a planned stage. The grammar specification drives the design of the
lexer, parser, AST and semantic model, so those are deliberately absent until it arrives.

## Pipeline

```
   source text
        │  (source.rs: SourceManager)
        ▼
   ┌─────────┐    tokens with Span
   │  Lexer  │──────────────────────────────────┐
   └─────────┘                                  │
                                                ▼
   ┌─────────┐    AST (arena of nodes)      ┌──────────┐
   │ Parser  │────────────────────────────►│ AST dump │  --dump-ast
   └─────────┘                             └──────────┘
        │
        ▼
   ┌──────────────┐   typed HIR (resolved names, inferred types)
   │ Semantic     │────────────────────────────────────────────┐
   │ analysis     │                                            │
   └──────────────┘                                            ▼
        │                                                diagnostics
        ▼
   ┌──────────────┐   llvm::Module (inkwell)
   │ IR lowering  │─────────────────────────────► module.verify()
   └──────────────┘                                     │
        │                                               ▼
        ▼                                    ┌──────────────────┐
   ┌──────────────┐  .ll / .o / exe           │ clang            │
   │  Driver      │─────────────────────────►│ (LLVM assembler  │
   └──────────────┘                          │  and linker)     │
        │                                    └──────────────────┘
        ▼
   link with runtime support library (libnscrt) + system libraries
```

### Stage contracts

| Stage | Input | Output | Fails by |
|---|---|---|---|
| Lexer | `&str` buffer | `Vec<Token>` with `Span` | error diagnostics, recovery at statement boundaries |
| Parser | token slice | `AstArena` + root `NodeId` | error diagnostics, panic-mode recovery to sync tokens |
| Sema | AST | typed HIR | error diagnostics, continues where safe |
| Codegen | HIR | `inkwell::module::Module` | internal invariant violation (a bug, not user error) |
| Driver | command line | object/executable | usage errors, propagated compilation failure |

## Design decisions

1. **Hand-written lexer and recursive-descent parser.** The grammar is small enough, and
   rich diagnostics (recovery, precise spans, "did you mean ...?") matter more than parser
   generator convenience. No parser generators are used.
2. **Spans are byte offsets.** Tokens and AST nodes store a `Span { file, start, end }`,
   never line/column pairs. Conversion happens lazily in `SourceManager`, which keeps
   copying cheap and avoids stale coordinates.
3. **Arena-allocated AST.** Nodes live in a `Vec<Node>` and are referred to by a small
   `NodeId`. In Rust this avoids pervasive `Box`/`Rc` and the borrow-checker friction of a
   pointer-linked tree, while keeping traversal and rewriting trivial.
4. **Explicit typed HIR.** Sema lowers the AST into a typed HIR, so codegen never
   re-implements name resolution or implicit conversions. If the language turns out small,
   this can collapse into a typed AST *only if* it stays simple.
5. **LLVM IR only in `codegen`.** The front end stays linkable and testable without a code
   generator. Every module is passed through LLVM's verifier before it is written out.
6. **`clang` as the assembler/linker driver.** Emitting textual IR (or a module) and
   invoking `clang` gives us LLVM's optimizer and the system linker without embedding
   either. Object emission, linking and `-O` flags all flow through one well-tested tool.
7. **Diagnostics are data.** `DiagnosticEngine` counts diagnostics; `render()` is a pure
   function returning the exact text. Neither throws nor exits. Exit codes belong to the
   driver.
8. **No `unsafe`.** The crate is `#![forbid(unsafe_code)]`; all LLVM interaction goes
   through inkwell's safe API.
9. **One shared runtime.** `runtime/` provides `libnscrt`, a C library implementing
   strings, arrays, printing and other primitives that generated code calls. Keeping it in
   C keeps the ABI and start-up simple and lets `clang` link it directly.

## Planned module layout

```
src/
  lexer.rs       Token, TokenKind, Lexer
  ast.rs         node arena, node kinds, AstDumper
  parser.rs      recursive-descent parser, recovery
  sema/          symbol tables, types, HIR lowering
  codegen/       HIR -> inkwell IR (types, functions, control flow)
  driver.rs      actions and pipeline orchestration (extended)
runtime/         libnscrt sources (C)
tests/e2e/       end-to-end cases; IR goldens checked with FileCheck-18
```

## Testing strategy

- **Unit tests** live next to the code under `#[cfg(test)]`: pure functions are tested
  directly, with no process spawning or filesystem access unless necessary.
- **Integration tests** live in `tests/`:
  - `tests/cli.rs` drives the real binary and asserts stdout/stderr/exit codes.
  - `tests/codegen.rs` builds IR with inkwell, prints it, and compiles it with `clang` to
    prove the back end works end to end.
- **End-to-end language tests** (`tests/e2e/`, planned): each case is a NovaSharp program
  plus expected output/diagnostics, driven from an integration test or the test script.
- **IR golden tests**: `// CHECK:` markers verified with `FileCheck-18` against
  `ai-nsc emit-ir`, catching codegen regressions without full execution.
- **Fuzzing** (Phase 6): `cargo-fuzz` targets for the lexer and parser.

## Error handling and limits

- Front-end errors never crash the compiler; codegen relies on sema guarantees and panics
  only on internal invariant violations (a compiler bug).
- A compilation unit is rejected if any error diagnostic was emitted.
- Recursion in parser/codegen is bounded by input depth; a nesting limit diagnostic is
  planned to avoid stack exhaustion on hostile input.
