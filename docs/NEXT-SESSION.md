# Handoff for the next session

## State

- Branch `master`; latest commit `72587df` "Diagnose uninitialized constants
  and illegal-value comparisons"; previous `0ec3a18` "Implement 'use' imports
  (§5.6)", `5fe9f0c` "Implement 'owner' access to shadowed outer-scope names
  (§6.4)", `8c0b507` "Validate constructor chains (§3.7.1)", `eafa0da`
  "Implement built-in pointer arithmetic",
  `a5cc9cf` "Fix high-severity conformance defects found by the spec audit" and
  `3a34c91` "Implement generics with compile-time monomorphization" (see
  `git log --oneline`). Working tree may be
  dirty while a concurrent session lands fixes — check `git status` before
  committing.
- Gates: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  — **172 tests green** (40 lib unit including 4 AST-clone tests, 34 CLI,
  2 codegen/LLVM, 85 end-to-end execution, 10 parser, 1 spec-example conformance).
- Build flags (required on this host):
  `CARGO_HOME=/tmp/ainsc-cargo LLVM_SYS_181_PREFIX=/usr/lib/llvm-18`
- CLI: `target/debug/ai-nsc check|emit-ir|emit-obj|build|run <file.ns>`.

## Where things live

- `src/lexer.rs`, `src/literal.rs` — tokens and literal decoding.
- `src/ast.rs` — arena AST; `src/ast_clone.rs` — deep subtree cloning used by
  monomorphization; `src/parser/{mod,expr}.rs` — recursive descent.
- `src/sema/{mod,types,check,generics}.rs` — declarations, layouts, name mangling,
  `TypeStore`, `SemaResult` (types/resolutions/functions/type_defs), body checks,
  generic templates and instantiations.
- `src/codegen.rs` — inkwell lowering: class metadata (typeinfo/vtables/interface
  tables), methods, construction, heap/dispose, operators, accessors, traps,
  runtime exceptions, `#if` branch selection.
- `docs/COVERAGE.md` — per-spec-section status (✅/🟡/⛔). Keep it current.
- `docs/AUDIT.md` — reproduced conformance defects; the ordered work list.
- `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/spec-notes/` (spec extracts,
  examples, known inconsistencies; §10 extract in `docs/spec-notes/generics.md`).

## How generics work now

The generics implementation is template-based with compile-time
monomorphization (§10):

- **Templates.** Pass 1 declares named types and records their generic parameters
  (`src/sema/mod.rs::declare_types`/`declare_named_type`); pass 2 records generic
  `fixed`/`linkable` function templates and, for generic type declarations, skips
  the body entirely. A template's signature/members/body are deliberately **not**
  resolved, because they mention generic parameters. Body checking skips them
  (templates have `body: None`).
- **Instantiation.** `src/sema/generics.rs::instantiate_type` /
  `instantiate_function` validate the argument count, check every constraint
  (`check_generic_constraints`), then deep-clone the declaration's AST subtree
  (`Sema::clone_declaration` → `src/ast_clone.rs`), grow the per-node
  `node_types`/`resolutions` tables, and resolve the clone under a substitution
  environment (`generic_env`, consulted by `resolve_type` via
  `lookup_generic_parameter`). Each instance therefore owns its nodes, fields,
  methods, accessors, operators and layouts.
- **Caching and names.** Instances are cached per
  `(declaration node, argument list)` (`type_instantiations` /
  `function_instantiations`), so repeated uses share one instance and distinct
  argument lists coexist. Instance qualified names include the arguments
  (`Box<integer(4)>`) and instance methods are mangled from that name plus the
  parameter spellings, so mangled symbols are per-instance.
- **Where monomorphization is triggered.** `check_monomorphization` is reached
  from type positions (`NamedType` with arguments), constructions, calls
  (`CallExpr.callee`), member constructions and `is` targets. Module-level
  `extension for Generic<T>` declarations are recorded as
  `PendingGenericExtension` and applied to existing and future instances
  (`register_generic_extension` / `apply_generic_extension`).
- **Passes skip templates.** Pass 2 (`resolve_declarations`) skips template bodies
  and members, so structure layouts are not computed for templates; vtable building
  (`build_vtables`), body checking (templates have `body: None`) and codegen
  (`prepare_struct_types`, `declare_functions`, `build_class_metadata`,
  `generate_functions`) skip declarations whose `generic_parameters` is non-empty.
  `finalize_variant_union_layouts` runs over the whole table but is a no-op for
  templates because their variant/union member lists were never resolved.
- **Recompute after bodies.** `Sema::run` calls `finalize_variant_union_layouts`
  and `build_vtables` a second time after body/initializer checking, because
  checking a body can create new instantiations.

### Gotchas future work must respect

1. **Never resolve a generic declaration directly.** Its members mention the
   parameters; only clones resolved under a `generic_env` are valid. When adding
   a path into `resolve_type`/member resolution, be sure it runs under the
   environment pushed by `instantiate_type`/`instantiate_function` (and popped
   afterwards).
2. **Any new pass that walks type definitions or functions must skip templates.**
   Test `generic_parameters.is_empty()` on `TypeDef`/`FunctionInfo` before
   touching layouts, vtables, metadata, mangled symbols or code generation.
   Instances have empty `generic_parameters` and instead carry `template:
   Some(...)`, `generic_arguments` and `generic_env`.

## Next steps

Work `docs/AUDIT.md` in severity order; every item has a reproduction. Fix a
group, add regression tests, keep the full suite green, then close its rows.

Done in `a5cc9cf`: audit #1–#6, #8–#10, #12, #18 (duplicate fields/enum
members/variant states/union members/function signatures/`Main`) and #19
(abstract instantiation, sealed bases) are fixed with regression tests; #7 is
partial (explicit `base.Construct(...)` works, but the §3.7.1 "first statement
must call the base constructor" diagnostic and the "derived class must declare a
constructor when the base one is non-trivial" diagnostic are still missing).
The fixes also removed a latent `type_id_for_node` collision between the
synthetic built-in exception declarations and the first AST node of a file.

1. **HIGH group is closed** (#1–#10 fixed). Remaining MED/HIGH #11 tail:
   `Type.SizeOf<T>()` (needs the compile-time API) and `p += i` compound
   assignment.
2. **MED items** in order: non-`this` `&`
   parameters, VLAs, array flattening/casts, heap inference/allocator, `dispose`,
   missing-return/`finally` diagnostics, `illegal`/`uninitialized`, variant
   elision/equality, and the warning subsystem (no `Severity::Warning` sites
   exist yet, so every spec-mandated warning is missing). The LOW items
   (#26–#31) are lower priority but cheap.
3. **Only then** the remaining `COVERAGE.md` 🟡/⛔ items: compile-time execution
   `#(...)`/`#{...}` (incl. use of generic parameters in compile-time code),
   `??` with `Optional`/`Result`, the Built-In Library, static members,
   `implicitable`, NOLAA, `&`/`*` reference expressions, function
   values/closures, and named/default argument lowering. §10 leftovers (generic
   methods with their own type parameters, array extension targets) are listed in
   `COVERAGE.md` §10.

## Conventions

- Never silently miscompile: unsupported constructs must produce an explicit
  "not implemented yet" diagnostic (`Sema::unsupported`).
- Malformed input must produce diagnostics, never hang: parser recovery must make
  progress on every iteration (a hang on malformed `match` input was fixed in
  `3a34c91`; keep regression tests for recovery loops).
- Draft spec: record divergences/tolerated ambiguities in
  `docs/spec-notes/_inconsistencies.md`; when the spec file changes, re-run the
  spec-example conformance test (`tests/spec_examples.rs`).
- Keep each round green: fmt + clippy `-D warnings` + full test suite before
  commit. Update `docs/COVERAGE.md` and close the matching `docs/AUDIT.md` rows
  in the same round.
