# Handoff for the next session

## State

- Branch `main`; last feature commit `761f461`.
- **Active goal: 100% Draft 1.16 conformance.** Authoritative specification:
  `docs/grammar/Draft 1.16.md` (1.153 and earlier are historical only).
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` — **429 tests green** (41 lib unit, 141 CLI, 2 codegen/LLVM,
  1 MCJIT probe, 2 JIT compile-time evaluation, 229 end-to-end execution,
  10 parser, 3 spec-example conformance).
- Spec tests read Draft 1.16 directly: 102 `novasharp` fences; all parse except
  five documented syntax fragments; every complete `Main` example compiles; the
  demonstration-only semantic-failure set is pinned
  (`3858, 4834, 4902, 6692, 6761, 6824, 7003, 8983, 10620`).

## Draft 1.16 migration so far

- Sections/anchors/examples moved from 1.153 to 1.16.
- Removed old `illegal` / `infinite` keywords and literals; added
  `float(4/8).NotANumber`, `float(4/8).Infinite`, negative infinity and the
  `is float(N).NotANumber` `FloatSpecialTarget` (codegen `fcmp uno`).
- Interfaces now declare contract members directly (no extension blocks); class
  body extensions implement them.
- Bare instance field names inside methods/extensions resolve through implicit
  `this` (reads, writes, lvalue, accessor backing).
- Type-name elision (§3.8.5) for enumeration members, union members and variant
  states in typed contexts.
- Global `fixed` constants may infer their type from a literal initializer.
- Existing 1.153 features retained: inline assembly full register barriers,
  Environment API/facts specialization, `#`/`&` type contracts/mangling,
  `scoped` duplicates, monomorphization base restriction, runtime default
  expressions, NOLAA conservative analysis, compile-time scalar/scalar-field
  objects, top-level compile-time `trap`/`throw`/`finally`.

## Remaining deviations (also listed in COVERAGE.md)

1. **Compile-time non-scalar execution (§11.4)**: thrown objects/catch bindings
   and hierarchy matching are now interpreted; constructor/destructor bodies of
   compile-time objects and non-scalar fields remain unimplemented.
2. **Interface constant contracts (§3.8.3)**: function/accessor/operator contracts
   work; non-function constants without initializers are not modelled as
   class-side constant contracts.

## Next steps (round order)

1. Interface constant contracts.
2. Compile-time object constructor/destructor and thrown object/catch bindings.
3. Re-run all spec examples with `ai-nsc check` (not only `Main` programs),
   close remaining documented-demo vs real-gap distinctions.
4. Final audit: COVERAGE/AUDIT/README/spec-notes consistency, remaining
   deviations and full gates.

## Where things live

- `src/lexer.rs`, `src/literal.rs` — tokens/literal decoding.
- `src/ast.rs`, `src/ast_clone.rs` — arena AST and subtree cloning.
- `src/parser/{mod,expr}.rs` — recursive descent.
- `src/sema/{mod,types,check,generics}.rs` — declarations, layouts, mangling,
  bodies, generics/constant + EnvironmentFacts specialization.
- `src/codegen.rs` — inkwell lowering (metadata/vtables/interfaces, heap,
  dispose, operators, accessors, traps, inline asm, UEFI helpers).
- `docs/COVERAGE.md` — Draft 1.16 section matrix and deviations.
- `docs/AUDIT.md` — reproduced conformance defects/fixes.
- `docs/CONFORMANCE.md` — concise final evidence report.
- `docs/spec-notes/` — spec extracts and known inconsistencies.

## Conventions

- Never silently miscompile: unsupported constructs produce an explicit
  "not implemented yet" diagnostic.
- Keep every round green (fmt, clippy `-D warnings`, full test suite) and update
  COVERAGE/AUDIT/NEXT-SESSION in the same round.
- When the spec changes, re-run `tests/spec_examples.rs` and update the closed
  documented-failure set, never hide a new failure.
