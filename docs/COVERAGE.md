# Spec coverage matrix

Status against `docs/grammar/Draft 1.12.md` (draft). Legend:

- ✅ implemented and covered by automated tests
- 🟡 implemented partially / with documented limitations
- ⛔ not implemented yet (diagnosed as "not implemented yet", never silently miscompiled)

Verification gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test` (156 tests: 40 lib unit including 4 AST-clone tests, 29 CLI, 2 codegen/LLVM,
74 end-to-end execution, 10 parser, 1 spec-example conformance), plus the CLI
`ai-nsc check/emit-ir/run` smoke path.

Audit-driven corrections landed in `a5cc9cf` (see `docs/AUDIT.md` for the register):
enumeration values/casts, ABI spellings of constructed types, `match` logical
control flow (`break`/`continue`/named labels) and user-defined operator patterns,
inner-`catch` propagation to outer traps, float→integer wrapping, array↔pointer
cast aliasing, raw `\x` byte escapes, sealed/abstract class checks, duplicate
declarations and duplicate `Main`. The rows below that were affected have their
notes updated accordingly.

Known conformance defects that are already reproduced are tracked separately in
`docs/AUDIT.md`; a ✅ below means the feature has tests, not that every defect in the
area is fixed.

| Spec area | Status | Notes / evidence |
|---|---|---|
| §2 lexical (tokens, comments, `@`, verbatim strings, encoding tags, `..`/`..!`) | ✅ | `src/lexer.rs` unit tests |
| Literal decoding (integer/float/character/string, encodings) | ✅ | `src/literal.rs` unit tests |
| §3.2 basic types, integer families, no implicit basic conversions | ✅ | e2e arithmetic/precedence tests |
| §3.5 value/reference semantics, class values are references | ✅ | e2e object model tests |
| §3.6.1 arrays (fixed dimensions), indexing, bounds check, array↔pointer casts | ✅ | e2e arrays; bounds raise `OutOfRangeException`; casts reference the same storage |
| §3.6.2 pointers | 🟡 | pointer types, member access, built-in pointer arithmetic (`p ± i`, `i + p`, `p - q` with pointee-size unit); `Type.SizeOf<T>()`, `p += i` and `&`/`*` reference expressions pending |
| §3.6.3 function types / calling conventions | 🟡 | signatures parsed and checked; indirect calls through pointers pending |
| §3.7.1 classes: layout, inheritance, vtable/RTTI, construction, destructors | 🟡 | layout, construction, virtual dispatch, RTTI, `base`, `base.Construct(...)`, sealed/abstract checks; constructor chains validated (§3.7.1); statics pending |
| §3.7.2 structures: exact layout | ✅ | layout rules + tests |
| §3.7.3 interfaces: contracts, dispatch, RTTI | ✅ | e2e interface dispatch/RTTI tests |
| §3.7.4 enumerations | ✅ | member values, implicit-to-underlying conversion, explicit integer↔enumeration casts |
| §3.7.5 variants | ✅ | state/payload layout, member construction, `is` + match patterns |
| §3.7.6 unions | ✅ | overlap layout, `aligned(N)`, member construction/punning |
| §3.7.7 base list | ✅ | one class + interfaces; monomorphized generic targets (`Class : Base<T>`, generic interface implementations) |
| §3.8.1 fields, field initializers | ✅ | initialization order tests |
| §3.8.2 accessors | 🟡 | extension/interface accessors, backing get/set, block/expression forms; free/static accessors pending |
| §3.8.3 extensions | ✅ | methods, accessors, operators on types; module-level `extension for Generic<T>` targets are instantiated with their type |
| §4.1–4.4 value/paren/cast/access expressions | ✅ | parser + e2e |
| §4.4.3 index access (array + indexer operator) | ✅ | bounds check; indexer get/set tests |
| §4.5 calls, overload resolution, default/named args | 🟡 | positional/default overload resolution; named args/function values pending |
| §4.6 construction, §4.7 member construction | ✅ | in-place construction tests |
| §4.8 `is` expressions (classes, interfaces, variants, special floats) | ✅ | RTTI e2e tests |
| §4.9 operators: overloads, derived comparisons, indexer, conversion | ✅ | operator e2e tests |
| §4.9.6/4.9.7 `&`/`*` reference expressions | ⛔ | diagnosed |
| §4.9.26 `??` | ⛔ | diagnosed (needs `Result`) |
| §4.9.27 `?:` conditional | ✅ | conditional expression e2e |
| §5 statements (blocks, locals, fixed, set/compound, if, while, do, for, match, break/continue, return, export) | ✅ | e2e statement tests; `match` is a logical control flow (`break`/`continue`/named labels) and pattern comparisons use operator overloads |
| §5.10.3/4 `throw`, `trap`/`catch`/`finally` | 🟡 | source-level + runtime exceptions; `finally` on `return`/`break`/`continue` pending; heap-failure exceptions pending |
| §6 names/modules/`use`, scopes, shadowing | 🟡 | modules/scopes resolved; `owner`/nested `owner` for shadowed outer names (reads and writes); `use` imports module members per scope (§5.6); multiple input files pending |
| §7/§13/§14 ABI: name mangling, class layout, launcher, exit code | ✅ | mangling unit tests, LLVM smoke + e2e exit codes |
| §8 functions: parameters, defaults, overload resolution, `#` const params | 🟡 | defaults/overloads; `#` compile-time parameters and default-argument call lowering pending |
| §9 modifiers (accessibility, scoped, readonly, implicitable, verbatim, manual, abstract, sealed, virtual, override, aligned, static) | 🟡 | virtual/override/sealed/aligned/static parsing; accessibility and some modifier semantics not enforced |
| §10 generics / monomorphization | ✅ | `src/sema/generics.rs`: templates for `class`/`structure`/`interface`/`variant`/`union` and `fixed`/`linkable` functions; kind constraints (`class`, `structure`, `interface`, `enumeration`, `variant`, `union`, `basic`) and named-type constraints with diagnostics; monomorphization in type positions, constructions, calls, member constructions and `is` targets; module-level `extension for Generic<T>` targets; instances cached per `(declaration node, argument list)` with their own mangled names. Remaining: generic methods declaring own type parameters, array extension targets, generic enumerations (spec-forbidden) — see below |
| §11.1–11.5 compile-time execution, `#(...)`, `#` params | ⛔ | diagnosed |
| §11.6 `#if` / `#else` | ✅ | constant evaluation, selected-branch-only checking/generation |
| §11.7 `#{...}` compile-time blocks | 🟡 | treated as blocks; compile-time variable constant propagation pending |
| §11.8 compiler API (`Type`, `Compiler`, `Platform`, NOLAA) | ⛔ | diagnosed |
| §12 memory model: locals, heap/dispose, NOLAA | 🟡 | heap/dispose; NOLAA analysis pending |
| §15 Built-In Library (Object, exceptions, Optional/Result, containers, iterators, compiler API) | 🟡 | exception class hierarchy only; rest pending |

### §10 generics: what is still not implemented

- Generic **methods** that declare their own type parameters (`fixed Foo<U>(...)` inside an
  extension) are not supported; calls report a diagnostic. Accessors and nested members may
  only borrow the enclosing declaration's parameters.
- **Array extension targets** (`extension for array(...)`) remain unsupported and are
  reported as "not implemented yet".
- **Enumeration declarations cannot be generic** — this is forbidden by the specification
  (§10.1) and unchanged; the parser does not accept generic parameters on `enumeration`.
- Compile-time use of generic parameters (inside `#(...)`/`#{...}`, §11.7) is deferred
  together with §11 compile-time execution.
- Monomorphization volume optimization (§14.3.2) is not implemented.

Open draft inconsistencies and tolerated gaps are recorded in
`docs/spec-notes/_inconsistencies.md`; the conformance test parses all examples from
`docs/grammar/Draft 1.12.md` (one non-program type fragment skipped). Reproduced
conformance defects are tracked in `docs/AUDIT.md`.
