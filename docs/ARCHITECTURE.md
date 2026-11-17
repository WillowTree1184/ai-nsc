# ai-nsc architecture

This document describes the intended structure of the NovaSharp compiler. The
skeleton in this repository is the foundation described under "Current state".

## Current state

```
ai-nsc
 ├── Base/SourceLocation      FileID + byte offset, source ranges
 ├── Base/SourceManager       file buffers, line/column mapping, excerpts
 ├── Base/Diagnostics         severity, formatting, counts, caret excerpts
 └── Driver/Driver            argument parsing, action dispatch, exit codes
```

Everything else is a planned stage. The grammar specification drives the design
of the lexer, parser, AST and semantic model, so those are deliberately absent
until it arrives.

## Pipeline

```
   source text
        │  (Base/SourceManager)
        ▼
   ┌─────────┐    tokens with SourceRange spans
   │  Lexer  │──────────────────────────────────┐
   └─────────┘                                  │
                                                ▼
   ┌─────────┐    AST (arena-allocated)    ┌──────────┐
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
   ┌──────────────┐   llvm::Module
   │ IR lowering  │─────────────────────────────► llvm::verifyModule
   └──────────────┘                                     │
        │                                               ▼
        ▼                                    ┌──────────────────┐
   ┌──────────────┐  .ll / .bc / .o / exe     │ LLVM back end    │
   │  Driver      │◄─────────────────────────│ (target machine) │
   └──────────────┘                           └──────────────────┘
        │
        ▼
   link with runtime support library (libnscrt) + system libraries
```

### Stage contracts

| Stage | Input | Output | Fails by |
|---|---|---|---|
| Lexer | file buffer | token stream + spans | error diagnostics, recovery at statement boundaries |
| Parser | token stream | AST + spans | error diagnostics, panic-mode recovery to sync tokens |
| Sema | AST | typed HIR | error diagnostics, continues where safe |
| Codegen | HIR | `llvm::Module` | internal invariant assertion (a bug, not user error) |
| Driver | command line | object/executable | usage errors, propagated compilation failure |

## Design decisions

1. **Hand-written lexer and recursive-descent parser.** The grammar spec is
   small enough, and rich diagnostics (error recovery, precise spans, "did you
   mean ...?") matter more than parser-generator convenience. `bison`/`flex`
   are available but intentionally unused.
2. **Spans are byte offsets.** Tokens store `SourceRange`, never line/column.
   Conversion happens lazily in `SourceManager`; this keeps copying cheap and
   survives any future source rewrites.
3. **Arena-allocated AST.** AST nodes are allocated from an arena owned by the
   parse result, never individually deleted. This removes ownership bugs from
   the front end.
4. **Explicit intermediate representation (HIR).** Sema lowers the AST to a
   typed HIR so codegen never has to re-implement name resolution or implicit
   conversions. If the language turns out small, this can collapse into a typed
   AST *only if* it stays simple.
5. **LLVM IR only in the codegen layer.** The front end remains linkable and
   testable without LLVM; `llvm::verifyModule` gates every module that leaves
   codegen.
6. **Diagnostics are data.** `DiagnosticEngine` counts and prints; it never
   throws or exits. Exit codes belong to the driver.
7. **One shared runtime.** `runtime/` builds `libnscrt`, the C library that
   implements strings, arrays, printing and other primitives the generated code
   calls. Keeping it in C keeps ABI and startup simple.
8. **Link with the system toolchain.** The driver shells out to `clang` (or the
   configured linker) with the emitted object; it does not embed a linker.

## Planned module layout

```
include/ainsc/
  Base/          SourceLocation, SourceManager, Diagnostics, (Token kinds)
  Lexer/         Lexer, Token
  Ast/           AST nodes, AstDumper
  Parser/        Parser, ParseResult
  Sema/          SymbolTable, Type, HIR
  Codegen/       Codegen (AST/HIR -> llvm::Module)
  Driver/        Driver, Options
runtime/         libnscrt (C)
tests/e2e/       end-to-end tests driven by scripts/run-e2e.py
```

## Testing strategy

- **Unit tests** (`tests/unit`): self-contained, in-tree harness, no external
  dependencies. One file per module.
- **End-to-end tests** (`tests/e2e`): each case is a NovaSharp program plus
  expected behavior (exit code, stdout, stderr diagnostics, optionally golden IR
  checked with `FileCheck-18`). A Python runner drives `ai-nsc` and diffs.
- **IR golden tests**: textual `// CHECK:` markers in NovaSharp programs check
  the emitted `.ll`; this catches codegen regressions without full execution.
- **Sanitizer preset**: `cmake --preset asan` for lexer/parser memory bugs.

## Error handling and limits

- Front-end errors never crash the compiler; codegen relies on sema guarantees
  and asserts on violations.
- A compilation unit is rejected if any error diagnostic was emitted.
- Recursion in parser/codegen is bounded by the input depth; a nesting limit
  diagnostic is planned to avoid stack exhaustion on hostile input.
