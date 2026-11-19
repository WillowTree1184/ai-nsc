# Handoff for the next session

## State

- Branch `main`; last feature commit `c2e41ef`.
- **Active goal: 100% Beta 1.1 conformance.** Authoritative specification:
  `docs/LanguageSpecification/docs/specification/zh-cn.md` (1.153 and earlier are historical only).
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` — **443 tests green** (41 lib unit, 146 CLI, 2 codegen/LLVM,
  1 MCJIT probe, 2 JIT compile-time evaluation, 238 end-to-end execution,
  10 parser, 3 spec-example conformance).
- Spec tests read Beta 1.1 directly: 102 `novasharp` fences; all parse except
  five documented syntax fragments; every complete `Main` example compiles; the
  demonstration-only semantic-failure set is pinned
  (`3858, 4834, 4902, 6692, 6761, 6824, 7003, 8983, 10620`).

## Beta 1.1 migration so far

- Sections/anchors/examples moved from 1.153 to Beta 1.1.
- Removed old `illegal` / `infinite` keywords and literals; added
  `float(4/8).NotANumber`, `float(4/8).Infinite`, negative infinity and the
  `is float(N).NotANumber` `FloatSpecialTarget` (codegen `fcmp uno`).
- Interfaces declare contract members directly (no extension blocks); class
  body extensions implement them.
- Bare instance field names inside methods/extensions resolve through implicit
  `this` (reads, writes, lvalue, accessor backing).
- Type-name elision (§3.8.5) for enumeration members, union members and variant
  states in typed contexts.
- Global `fixed` constants infer types and drive top-level `#if`.
- Function-typed fields accept `Block` initializers; `linkable` variables emit
  external globals.
- Compile-time thrown objects, catch bindings and class-hierarchy matching are
  interpreted; scalar-field compile-time objects support fields/dispose/`is`.
- Existing 1.153 features retained: inline assembly full register barriers,
  Environment API/facts specialization, `#`/`&` type contracts/mangling,
  `scoped` duplicates, monomorphization base restriction, runtime default
  expressions, NOLAA conservative analysis.

## Remaining deviations (also listed in COVERAGE.md)

1. **Compile-time non-scalar execution (§11.4)**: constructor `this.Method()`
   calls, scalar-structure copies, one-dimensional scalar array copies, string
   byte-array copies and enumeration copies are implemented; constructor/
   destructor exceptions/side effects, non-scalar/pointer fields and union/
   variant copies remain.
2. **Interface constant contracts (§3.8.3)**: function/accessor/operator contracts
   work; non-function constants without initializers are not modelled as
   class-side constant contracts.
- The full non-`Main` example scan additionally shows function-valued extension
  constants and some accessor/`this`-scope examples still diagnosing; these are
  recorded honestly in `docs/CONFORMANCE.md` and need resolution for literal
  100%.

## Next steps

1. Compile-time object constructor/destructor body interpretation, lifecycle
   ordering, copy-to-runtime restrictions and nested non-scalar fields.
2. NOLAA/automatic-dispose compile-time exclusion checks.
3. Remaining Beta 1.1/1.16 example gaps from the full `check` scan (accessor/`this`
   scope, function-valued extension constants are now restricted per spec).
4. Final audit: COVERAGE/AUDIT/README/spec-notes consistency and full gates.

## Where things live

- `src/lexer.rs`, `src/literal.rs` — tokens/literal decoding.
- `src/ast.rs`, `src/ast_clone.rs` — arena AST and subtree cloning.
- `src/parser/{mod,expr}.rs` — recursive descent.
- `src/sema/{mod,types,check,generics}.rs` — declarations, layouts, mangling,
  bodies, generics and EnvironmentFacts specialization.
- `src/codegen.rs` — inkwell lowering (metadata/vtables/interfaces, heap,
  dispose, operators, accessors, traps, inline asm, UEFI helpers).
- `docs/COVERAGE.md` — Beta 1.1 section matrix and deviations.
- `docs/AUDIT.md` — reproduced conformance defects/fixes (197 entries).
- `docs/CONFORMANCE.md` — concise final evidence report.
- `docs/spec-notes/` — spec extracts and known inconsistencies.

## Conventions

- Never silently miscompile: unsupported constructs produce an explicit
  "not implemented yet" diagnostic.
- Keep every round green (fmt, clippy `-D warnings`, full test suite) and update
  COVERAGE/AUDIT/NEXT-SESSION in the same round.
- When the spec changes, re-run `tests/spec_examples.rs` and update the closed
  documented-failure set, never hide a new failure.
