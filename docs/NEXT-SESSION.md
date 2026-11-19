# Handoff for the next session

## State

- Branch `main`; last feature commit `6194cdf`.
- **Active goal: 100% Draft 1.153 conformance, no more no less.** The
  authoritative specification is `docs/grammar/Draft 1.153.md`; Draft 1.152 is
  historical only.
- Gates: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  — **412 tests green** (41 lib unit, 139 CLI, 2 codegen/LLVM, 1 MCJIT probe,
  2 JIT compile-time evaluation, 214 end-to-end execution, 10 parser, 3
  spec-example conformance). The spec-conformance tests read `Draft 1.153.md`:
  all 97 `novasharp` examples parse (the bare inline-assembly block and the bare
  function-type fragment are recorded as syntax fragments), and every whole
  program that passes semantic analysis compiles end to end.
- Draft 1.153 baseline (round 23): spec path/whitelists switched, 1.152 parser
  tolerances removed (expression-bodied function literals, `case pattern:`
  single statements), inline-assembly examples temporarily skipped as fragments.
- Draft 1.153 semantic fixes (round 24): duplicate overload signatures rejected,
  mangling contract markers, repeated modifiers rejected, monomorphization base
  restricted to identifiers, `Type.SizeOf` returns 64-bit values.
- Draft 1.153 semantic fixes (round 25): `Environment` API replaces `Platform`
  (Current/Host/Target, architecture facts, string queries), with tests for host,
  target and cross-target architecture; `Platform` is no longer accepted.
- Draft 1.153 new feature (round 26): inline assembly (§5.2) parsed and lowered,
  with x86_64 `mov`/`add` e2e, escapes and diagnostics; assembly spec examples now
  parse. Remaining assembly limitation: the full no-clobber register barrier (COVERAGE
  deviation 14); target-syntax validation now happens in `check`.
- Draft 1.153 semantic fix (round 30): function types carry per-parameter
  `#`/`&` contracts; contracts participate in type equality and type spelling
  (and therefore in mangling).
- Draft 1.153 verification (round 27): default parameter expressions are
  caller-side runtime values (no compile-time-constant requirement) and are
  evaluated per call; Environment kind/architecture value queries are
  compile-time constants; assembly objects emit for x86_64, aarch64 and
  riscv64.
- **Draft 1.153 new work still open**:
  1. §5.2 inline assembly: implemented (lexer/parser/placeholders/escapes/
     clobbers/sema/LLVM InlineAsm/x86 e2e). Remaining: target-syntax validation
     during `check` (currently LLVM object emission rejects invalid text) and the
     strongest no-clobber register-barrier semantics (deviation 14/15).
  2. §11.9.3 `Environment` API: implemented (Current/Host/Target facts,
     `PlatformIs`/`ArchitectureIs`, host/target architecture, `Platform` removed).
     Remaining: a materialised `EnvironmentFacts`/`Environment` module declaration
     in the built-in type space (queries are compile-time folded) and the
     `#if` direct-call-only nuance for `#()` wrapping. EnvironmentFacts structure
     and Current/Host/Target values plus the Environment module are materialised
     (AUDIT #174/#175/#176); remaining: queries on facts values whose origin is
     not statically tracked are explicitly diagnosed.
  3. §8.5/§13.2 `#`/`&`: participate in type equality but not overload
     signature; only-contract overloads forbidden; mangling hash includes
     contract markers.
  4. §11.7 default value expressions no longer need to be compile-time
     constants; they are caller-side behavior.
  5. §9.5 duplicate `scoped` forbidden; §10.2 monomorphization base must be an
     `Identifier`; §11.9.1 `SizeOf` returns a 64-bit value (32-bit targets may
     use the low 32 bits).
  6. §5.2-§5.11 section renumbering: COVERAGE/AUDIT anchors, spec-notes and
     generated EBNF/example snapshots still need a mechanical 1.153 refresh.
- Priority: NOLAA full implementation (`docs/NOLAA.md`). Direct/pointer/field/
  pointer-member/container-element aliases, path-sensitive dispose state,
  interprocedural return ownership and direct parameter-disposal transfer are
  implemented; last-direct-demand safe points are emitted per statement. Next is
  dominator/CFG placement for nested paths + read-only parameter summaries +
  field-write/escape fixed point.
- Older remaining gaps (still valid): compile-time non-scalar heap, throw/trap
  and `is` match patterns.
- Targets: x86_64/aarch64/riscv64; i686 build+run on x86_64 hosts; armv7
  cross-linked with `--sysroot` + lld and run under qemu-arm; x86_64 UEFI builds
  a freestanding EFI application (qemu+OVMF boot test). riscv32 is object-only
  (no 32-bit RISC-V sysroot on this host).
- Compile-time execution (§11): top-level `#{}`/`#if`/`#()`, constants,
  assignments, control flow, match/do-while, scalar compile-time heap, ordinary
  and recursive compile-time functions, external linkable calls through the host
  JIT. Remaining §11 gaps listed above.
- Built-in library: `Object`, `CallingConvention`, `PlatformKind`, exceptions,
  `Optional`, `Result`, `IIterator`, `IIterable`, `IAllocator`,
  `DefaultAllocator`, `Type.SizeOf`, `Compiler.Log*/Assert`, `NOLAA.*`; the
  `Environment` module, `ArchitectureKind`/`EnvironmentKind`/`EnvironmentFacts`
  are materialised (remaining limitation: queries on untracked facts values).
- Docs: `docs/COVERAGE.md` maps every spec section to status; `docs/AUDIT.md`
  tracks 177 reproduced defects. `docs/spec-notes/_inconsistencies.md` was
  rewritten for 1.153.

## Remaining backlog (updated through round 40)

Recent rounds (108-119) fixed specification-conformance defects found by
auditing the specification instead of working the large backlog items; the
backlog below is therefore unchanged in kind but now annotated with status and
size. Each round also keeps the docs' audit register in sync.

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | `#{}` full compile-time execution | **mostly done** | Constant declarations, assignments (incl. compound), `if`/`while`/`for`-range with break/continue, `#if` and ordinary/recursive function calls execute at compile time; blocks with run-time statements are lowered as run-time equivalents with the spec-mandated side-effect warning and the §11.7 boundary diagnostic. Remaining: compile-time array/iterable execution and non-integer values. Size: small/medium. |
| 2 | NOLAA precision (general CFG safe points, full alias analysis) | **acceptable conservative** | All safety requirements are met: block-end/return/break/continue safe points, alias/store demands as escapes, Bind groups, AssumeNoDemandNext, linkable-call abandonment. Remaining work is earlier-dispose precision (optimization, not correctness). Size: large/optional. |
| 3 | Array-extension indexer operators | **likely not applicable** | Array index access is a language built-in (§3.6.1/§4.4.3); extension indexers on array targets are diagnosed instead of silently ignored. Needs a spec clarification before changing. Size: small. |
| 4 | 32-bit targets (i686/armv7/riscv32) | **i686 + armv7 done; riscv32 environment-limited** | i686 runs natively, armv7 under qemu-arm (heap classes, interfaces, arrays, traps, header dispatch verified). riscv32: qemu-riscv32 exists but there is no 32-bit RISC-V sysroot/libc on this host, so it stays object-only/diagnosed. Size: small. |
| 5 | Foreign-target linking (`build`/`run` with `--target`) | **open, environment-dependent** | Requires a target sysroot/cross toolchain; `emit-obj` works for x86_64/aarch64/riscv64. Size: large/external. |
| 5b | JIT-based compile-time execution (user-approved) | **done (host)** | MCJIT evaluator wired into phase B: function bodies checked before compile-time execution, live `Resolution::TopLevelConstant`, address-based calls, integer args/results <=64 bits. Host uses JIT (float/loop bodies verified, 9/2 -> 4); cross targets keep the integer interpreter. Remaining: wider signatures (128-bit, floats as args/results) if needed. |
| 6 | Monomorphization volume optimization (§14.3.2) | **out of scope for correctness** | Constant/literal pooling is delegated to LLVM, which merges identical read-only data; no conformance requirement forces hand-written pooling. |
| 7 | Conformance audit | **ongoing** | 105 defects recorded (see `docs/AUDIT.md`); the audit continues each round. |
| 8 | ~~HIGH: heap-of-structure miscompile (§12)~~ | **fixed** | Cause: return-value disposal ran before evaluating the returned expression, freeing heap units that were then read. Fixed by evaluating first, spilling only when disposals exist (#105). |

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
