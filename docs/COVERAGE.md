# Spec coverage matrix

Status against `docs/grammar/Draft 1.12.md` (draft). The specification is the
source of truth and evolves; the matrix records what the compiler implements and
what it diagnoses instead of miscompiling.

Legend:

- ✅ implemented and covered by automated tests
- 🟡 implemented with documented limitations
- ⛔ not implemented yet (diagnosed as “not implemented yet”, never silently
  miscompiled)

## Verification

Gates run every round:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`cargo test` currently runs **234 tests**: 40 lib unit tests (lexer, literal,
mangling, AST clone), 54 CLI behaviour tests, 2 LLVM codegen tests, 125
end-to-end execution programs, 10 parser tests and **2 spec-conformance tests**
that read `docs/grammar/Draft 1.12.md` directly:

1. every `novasharp` fence in the specification parses (80+ examples);
2. every whole-program example that passes semantic analysis is compiled end to
   end by the CLI to an executable without diagnostics (15+ examples; examples
   with elided bodies or deliberately invalid code are skipped).

Smoke path: `ai-nsc check|emit-ir|emit-obj|build|run` with `--target`,
`--opt`, `--dump-tokens/--dump-ast`, single or multiple input files.

Conformance defects that were reproduced are tracked in `docs/AUDIT.md`
(60 entries, all fixed/partial with the commit that closed them). A ✅ below
means the area has tests, not that every conceivable defect is ruled out.

## Matrix

| Spec area | Status | Notes / evidence |
|---|---|---|
| §2 lexical: tokens, comments, `@` forced keywords, verbatim strings, encoding tags, `..`/`..!` | ✅ | lexer unit tests |
| Literal decoding: integer/float/character/string, encodings, raw `\x` bytes | ✅ | `src/literal.rs` unit tests |
| §3.1–3.2 type constructors, basic types, no implicit basic conversions | ✅ | e2e arithmetic, casts, comparison tests |
| §3.3–3.5 value/reference semantics, `&` by-reference parameters, scoped storage | ✅ | by-ref parameter and alias e2e tests |
| §3.6.1 arrays: fixed layout, indexing bounds, literals/repeat/range, array↔pointer casts, array parameter expansion, **VLA** | ✅ | e2e arrays/bounds/VLA tests; multi-dimensional VLAs with a leading runtime dimension supported; non-leading runtime dimensions diagnosed |
| §3.6.2 pointers: member access, `p ± i`, `p - q`, casts, `&`/`*` expressions | ✅ | e2e pointer tests |
| §3.6.3 function types, `wildcard`, calling conventions | ✅ | function value/closures tests; C/SystemV/Win64 LLVM conventions, others diagnosed |
| §3.7.1 classes: layout, inheritance, vtable/RTTI, construction, destructors, statics, abstract/sealed/static class | ✅ | class e2e tests, constructor-chain checks |
| §3.7.2 structures: exact layout, `aligned(N)` packing | ✅ | layout e2e tests |
| §3.7.3 interfaces: contracts, dispatch, RTTI, accessors, generics | ✅ | interface dispatch/RTTI tests |
| §3.7.4 enumerations: members, underlying type, casts | ✅ | enumeration e2e tests |
| §3.7.5 variants: state layout, payload construction, `is`/match, equality | ✅ | variant e2e tests |
| §3.7.6 unions: overlap layout, member offsets via `aligned`, member construction | ✅ | union e2e tests |
| §3.7.7 base lists: one class + interfaces, generic bases | ✅ | generics/bases tests |
| §3.8.1 fields, initializers (type-checked in declaring scope), readonly, accessibility | ✅ | field/initializer/readonly/accessibility tests |
| §3.8.2 accessors: extension/interface accessors, backing get/set, expression/block bodies, static accessor alias | ✅ | accessor e2e tests; static accessors with explicit members diagnosed |
| §3.8.3 extensions: members, extension-scoped functions, generic extension targets | ✅ | extension e2e tests |
| §4.1–4.4 value/parenthesized/cast/access expressions | ✅ | parser + e2e |
| §4.4.3 index access (arrays, VLA, indexer operators) | ✅ | bounds check raises `OutOfRangeException` |
| §4.5 calls: overloads, defaults, named arguments, function values, closures, nested functions, function pointers | ✅ | call/closure e2e tests |
| §4.6–4.7 construction and member construction | ✅ | construction tests |
| §4.8 `is` expressions: classes, interfaces, variants, special floats | ✅ | RTTI and `is illegal/infinite` tests |
| §4.9 operators: overloads, derived comparisons, indexer, conversion, `implicitable`, user `??` | ✅ | operator/implicit conversion tests |
| §4.9.6/4.9.7 `&`/`*` reference expressions | ✅ | pointer/reference tests |
| §4.9.26 `??` with `Optional`/`Result` and all continuations | ✅ | coalescing + user `operator ??` tests |
| §4.9.27 `?:` conditional expression | ✅ | conditional e2e |
| §5 statements: blocks, locals/fixed, set/compound, if, while/do, for, match, break/continue, return, trap/finally, throw, dispose, export/use | ✅ | statement e2e tests; `finally` executes on fall-through and normal `return` paths (rejected `return` inside `finally`) |
| §5.6 `use` imports (per scope, later imports first, multi-file) | ✅ | use tests |
| §5.9 `match`: patterns, guards, logical control flow (break/continue/labels) | ✅ | match e2e tests |
| §5.9.5 `for`: arrays, VLA, ranges, `&` element alias, `IIterator`/`IIterable` targets | ✅ | for e2e tests |
| §6 names/scopes: modules, shadowing, `owner` chains, imports | ✅ | scope/owner/use tests |
| §7 modules | ✅ | module e2e tests |
| §8 functions: parameters, defaults, by-ref, array parameter sugar, `#` compile-time constants, nested functions | ✅ | parameter/call tests; `#` on functions, methods and operators |
| §9 modifiers: accessibility, `scoped`, `readonly`, `implicitable`, `manual`, `abstract`/`sealed`/`virtual`/`override`, `aligned`, `static`, `verbatim` | 🟡 | all enforced, including `verbatim` (optnone functions, runtime-evaluated storages, compile-time evaluation rejected inside verbatim) |
| §10 generics / monomorphization | 🟡 | types/functions/constraints/extension-for-generic/caching/mangling; including methods that declare their own type parameters and array extension targets (methods); remaining: array extension indexers/derived operators and monomorphization-volume optimization (§14.3.2, not implemented) |
| §11.1–11.7 compile-time: `#(...)`, `#if`/`#else`, `#{}` blocks (incl. top-level and cross-file order), `#` constant parameters | ✅ | compile-time e2e tests; blocks are lowered as runtime equivalents of compile-time results (semantics-preserving approximation) |
| §11.8 compiler API: `Type.SizeOf<T>`, `Compiler.LogNote/LogWarning/LogError/Assert`, `Platform.Is`, `NOLAA.Abandon/Bind/AssumeNoDemandNext` | ✅ | compiler API tests |
| §12 memory model: locals/heap/dispose/destructors, `manual`, custom `IAllocator` + `DefaultAllocator`, NOLAA (double-dispose/early-dispose/never-disposed warnings and conservative auto-insertion) | ✅ | heap/dispose/allocation/NOLAA tests; NOLAA analysis is deliberately conservative (escapes are exempt) |
| §13 ABI: name mangling (§13.2), layouts, calling conventions, type info/vtables/interface tables | ✅ | mangling unit test + IR smoke tests; cross-target conventions beyond host diagnosed |
| §14 startup: launcher, static/runtime initializers, exit code = Main return | ✅ | launcher IR/e2e exit-code tests |
| §15 Built-In Library: `Object`, `CallingConvention`, `PlatformKind`, exception hierarchy with `InnerException`/constructors, `Optional<T>`, `Result<TValue; TAlternative>`, `IIterator<T>`, `IIterable<T>`, `IAllocator`, `DefaultAllocator`, compile-time library | ✅ | builtin library e2e tests; containers are not part of the spec's §15.2 |

## Known deviations / remaining backlog

These are deliberate, documented limitations rather than silent miscompiles:

1. **Compile-time blocks**: integer `#(...)` end products are folded at code
   generation (#64), but `#{}` blocks are still lowered as runtime equivalents;
   generated code is observationally equivalent rather than reduced to solely
   the required end products (§11.4 exact wording).
2. **Non-leading runtime array dimensions** are diagnosed (multi-dimensional
   VLAs with a leading runtime dimension are supported).
5. **`verbatim`** is parsed/accepted but has no distinct semantics yet.
6. **NOLAA** is a conservative approximation: warnings and block-end
   auto-dispose; arbitrary CFG safe-point insertion and alias/strongly-connected
   component analysis are not implemented.
7. **Monomorphization volume optimization** (§14.3.2) is not implemented.
8. **Target selection**: code generation targets the host ABI; calling
   conventions the host backend cannot represent are diagnosed.
9. **Platform queries** in the compile-time library answer for the host target.
11. Container/iterator utilities beyond the specification's §15.2 (dynamic
    arrays, lists, maps) are not provided.
