# Handoff for the next session

## State

- Branch `main`; last feature commit `06bdfd7`
- **Active goal: 100% draft-1.152 conformance, no more no less** (no
  compatibility syntax beyond the draft; no unimplemented normative rules).
  `git status` is clean.
- Gates: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  — **378 tests green** (41 lib unit including the lexer longest-match test, 116 CLI,
  2 codegen/LLVM, 1 MCJIT probe, 2 JIT compile-time evaluation, 203 end-to-end
  execution, 10 parser, 3 spec-example conformance). The spec-conformance tests
  read `Draft 1.152.md`: all 91 `novasharp` examples parse, and every whole-program
  example that passes semantic analysis is compiled end to end.
- Targets: x86_64/aarch64/riscv64; **i686** build+run on x86_64 hosts (verified
  end to end) and **armv7** cross-linked with `--sysroot` + lld and executed under
  qemu-arm; **x86_64 UEFI** builds a freestanding EFI application (qemu+OVMF boot
  test). All are covered by CLI tests. riscv32 is object-only (no 32-bit RISC-V
  sysroot on this host); aarch64 UEFI is not supported yet. UEFI has no platform
  `DefaultAllocator` (allocators are the developer's responsibility).
- Compile-time execution (§11): top-level `#{}` blocks run at compile time
  (constants, assignments incl. compound, if/while/for-range, break/continue,
  `#if`, ordinary + recursive function calls, external linkable calls through the
  host JIT; strict constant predicate for `#` parameters and array dimensions;
  compile-time errors abort execution immediately). On the host target the full language is
  executed through LLVM's MCJIT (LLVM 18 has no IR interpreter); cross targets use
  the integer interpreter.
- Built-in library (§15) complete: `Object`, `CallingConvention`, `PlatformKind`,
  the exception hierarchy with `InnerException`/constructors, `Optional<T>`,
  `Result<TValue; TAlternative>`, `IIterator<T>`, `IIterable<T>`, `IAllocator`,
  the platform `DefaultAllocator`, and the compile-time library
  (`Type.SizeOf`, `Compiler.Log*`/`Assert`, `Platform.Is`, `NOLAA.*`). Containers
  are intentionally not provided (§15.2 does not define them).
- Docs: `docs/COVERAGE.md` maps every spec section to ✅/🟡/⛔ with evidence and
  deliberate deviations; `docs/AUDIT.md` tracks the 151 reproduced defects (all
  fixed or marked partial with the fixing commit). Draft 1.152 changes are covered:
  `as` conversions, circular shifts, directional ranges, inferred `for` binding
  types, `@` markers, `for` without a binding, contextual keyword names.
- Draft 1.152 changes: lazy two-phase `#if` evaluation (§11.7) is implemented
  (immutable `fixed` forward bindings, settlement rounds, locked branches,
  no-progress diagnostics; oscillation/fixed-point detection was removed by the
  draft and is intentionally absent). Top-level `#()` statements execute in the
  phase-A/phase-B ordering and forward bindings are shared across modules.
  Residual limitation: non-API top-level `#()` expressions run in the second
  pass rather than the startup pass.
- Remaining gaps: keyword shadowing that disables a keyword meaning
  (§2.4/§6.4); compiler-defined platform extension names (§11.8.3 permits but
  does not require them, so only the standard names are matched). Compile-time
  API scoping, `Type.SizeOf` pointer width, accessor `this` rules and
  `array(Name: Type) T` expansion are implemented and tested.
- Build flags (required on this host):
  `CARGO_HOME=/tmp/ainsc-cargo LLVM_SYS_181_PREFIX=/usr/lib/llvm-18`
- CLI: `target/debug/ai-nsc check|emit-ir|emit-obj|build|run [--target=<triple>]
  [--sysroot=<path>] <file.ns> ...`.

## Remaining backlog (status as of round 128)

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
