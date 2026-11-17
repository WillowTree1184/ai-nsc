# Handoff for the next session

## State

- Branch `master`, clean tree, latest commit `51bd259` (see `git log --oneline`).
- Gates: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  — **100 tests green** (36 unit, 9 CLI, 2 LLVM smoke, 42 end-to-end execution,
  10 parser, 1 spec-example conformance).
- Build flags (required on this host):
  `CARGO_HOME=/tmp/ainsc-cargo LLVM_SYS_181_PREFIX=/usr/lib/llvm-18`
- CLI: `target/debug/ai-nsc check|emit-ir|emit-obj|build|run <file.ns>`.

## Where things live

- `src/lexer.rs`, `src/literal.rs` — tokens and literal decoding.
- `src/ast.rs` — arena AST; `src/parser/{mod,expr}.rs` — recursive descent.
- `src/sema/{mod,types,check}.rs` — declarations, layouts, name mangling,
  `TypeStore`, `SemaResult` (types/resolutions/functions/type_defs), body checks.
- `src/codegen.rs` — inkwell lowering: class metadata (typeinfo/vtables/interface
  tables), methods, construction, heap/dispose, operators, accessors, traps,
  runtime exceptions, `#if` branch selection.
- `docs/COVERAGE.md` — per-spec-section status (✅/🟡/⛔). Keep it current.
- `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/spec-notes/` (spec extracts,
  examples, known inconsistencies).

## Highest-priority gap: generics / monomorphization

Spec §10 and §3.7.7 require generic types/functions to be monomorphized before
use. Current status: parser accepts `<T; U>` and `Name<Arg; Arg>` syntax; semantic
analysis reports "not implemented yet".

Suggested implementation path:

1. Add a substitution environment to `Sema` (`generic_env: Vec<HashMap<String, TypeId>>`)
   consulted by `resolve_type` for `Name` nodes.
2. Pass 1: for generic declarations, create a *template* `TypeDef` and record the
   declaration node + parameter names (extend `TypeDef` with
   `generic_parameters: Vec<String>`).
3. Monomorphization: on `Name<Arg;...>`, canonicalize the argument list (interned
   `TypeId`s), look up/instantiate a `TypeDef` keyed by `(declaration NodeId,
   Vec<TypeId>)`:
   - create the `TypeDef` with a mangled qualified name (e.g. `Box<integer(1)>`),
   - re-run the existing member/extension resolution with `generic_env` pushed,
   - cache and reuse the instantiation.
4. Generic functions: instantiate `FunctionInfo` per argument list with the same
   env; bodies need per-instance type/resolution storage because `node_types` and
   `resolutions` are per-AST-node arrays. Two viable options:
   - clone the AST subtree per instantiation (preferred: no key changes), or
   - add `generic_node_types`/`generic_resolutions` keyed by `(function, node)` and
     route `set_node_type`/resolution writes through helpers.
5. Include the monomorphized argument spellings in the name-mangling parameter
   list (already handled by `mangle_function` once type ids exist).
6. Tests: generic structure with a method, generic function with `T` parameter and
   return, two instantiations of the same generic coexisting in one program,
   constraint diagnostics.

## Next gaps after generics (see `docs/COVERAGE.md`)

- Compile-time execution: `#(...)`, `#{...}` with constant propagation,
  `Type.SizeOf<T>()`, `Platform`/`CompileTimeError` compiler API.
- `??` operator + `Optional`/`Result`; complete Built-In Library (Object,
  containers, iterators, exception hierarchy methods).
- `implicitable` implicit conversion invocations; NOLAA; static members.
- `&`/`*` reference expressions, function values/closures, named/default argument
  lowering; `finally` on `return`/`break`/`continue`; heap allocation-failure
  exceptions.

## Conventions

- Never silently miscompile: unsupported constructs must produce an explicit
  "not implemented yet" diagnostic (`Sema::unsupported`).
- Draft spec: record divergences/tolerated ambiguities in
  `docs/spec-notes/_inconsistencies.md`; when the spec file changes, re-run the
  spec-example conformance test (`tests/spec_examples.rs`).
- Keep each round green: fmt + clippy -D warnings + full test suite before commit.
