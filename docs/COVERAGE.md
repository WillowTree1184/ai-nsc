# Draft 1.162 coverage matrix

Status against `docs/grammar/Draft 1.162.md`. The specification is the source of
truth; this matrix records what the compiler implements and what it diagnoses
instead of miscompiling.

Legend: ✅ implemented and tested · 🟡 implemented with documented limitations ·
⛔ not implemented (diagnosed, never silently miscompiled).

## Verification

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`cargo test` currently runs **435 tests**: 41 lib unit, 144 CLI/driver,
2 LLVM codegen, 1 MCJIT probe, 2 JIT compile-time evaluation, 228 end-to-end
execution, 10 parser and 3 spec-example conformance tests. The spec tests read
`Draft 1.162.md` directly: 102 `novasharp` fences, all parse except five documented
syntax fragments, every `Main` example that is a complete program compiles end
to end, and the exact set of demonstration-only `Main` examples failing
semantics is pinned (`3858, 4834, 4902, 6692, 6761, 6824, 7003, 8983, 10620`).

## Matrix

| Spec area | Status | Evidence / limitations |
|---|---|---|
| §2.1–2.6 source text, lexical rules, identifiers, keywords | ✅ | driver/lexer/parser tests; `illegal`/`infinite` are ordinary names in 1.162 |
| §2.7.1–2.7.3 integer/float literals and numeric literal inference | ✅ | literal unit tests and literal-family overload tests; defaults follow 1.162 |
| §2.7.4–2.7.10 float/bool/char/string/encoding/escapes | ✅ | lexer/literal/e2e tests |
| §2.7.11 arrays, ranges, array literals | ✅ | e2e arrays/bounds/VLA/range tests; multi-dimensional VLA limits diagnosed |
| §2.7.12 functions, closures, captures | ✅ | function value/closure/capture e2e |
| §2.7.13 `uninitialized` | ✅ | storage initializer tests; constants reject it |
| §3.1–3.2 type constructors, basic types, no implicit basic conversions | ✅ | constructor/cast/comparison e2e |
| §3.3 float special values (`NotANumber`, `Infinite`, negative infinity, `is`) | ✅ | parser/sema/codegen for `float(N).NotANumber/Infinite`; `is float(N).NotANumber` is `fcmp uno`; NaN comparisons follow IEEE 754; match NaN patterns; e2e |
| §3.4–3.6 value/reference semantics, `&` parameters, references | ✅ | by-ref/alias e2e |
| §3.7.1 arrays: layout, bounds, casts, parameter expansion, VLA | ✅ | array/VLA e2e; runtime dimensions in function bodies |
| §3.7.2 pointers: member access, arithmetic, casts, `&`/`*` | ✅ | pointer e2e |
| §3.7.3 function types, wildcard, calling conventions | ✅ | function-value/closure tests; supported conventions lowered, others diagnosed |
| §3.8.1 classes: layout, inheritance, vtable/RTTI, construction, destructors, statics, abstract/sealed | ✅ | class e2e; header vtable/typeinfo/allocator; explicit-allocator returned instances disposable |
| §3.8.2 structures: layout, `aligned(N)` | ✅ | layout e2e |
| §3.8.3 interfaces: direct contract members, dispatch, RTTI, accessors, generics, inheritance | ✅ | interface bodies declare contracts directly; class-body extensions implement them; interface value constants are instance contracts dispatched through interface dispatch constant slots (missing/static/type-mismatch implementations diagnosed); implicit instance fields resolve through `this` |
| §3.8.4 enumerations | ✅ | value/layout/cast/member tests |
| §3.8.5 type-name elision (enum/union/variant) | ✅ | enum bare members in typed contexts; union `Member(expr)` and variant `State`/`State(expr)` elision; e2e |
| §3.8.6 variants | ✅ | state/payload layout, construction, is/match, equality tests |
| §3.8.7 unions | ✅ | overlap layout, member construction/read tests |
| §3.8.8 base lists | ✅ | class/interface base and generic-base tests |
| §3.9.1 fields and initializers | ✅ | expression initializers, access/readonly/layout, and function-typed field `Block` bodies (synthesized function literal) |
| §3.9.2 accessors | ✅ | extension/interface/free accessors, backing get/set, expression/block bodies, static alias |
| §3.9.3 extensions | ✅ | extension methods/functions, generic/array targets, class-body interface implementation |
| §4.1–4.4 expressions, access, index access | ✅ | parser/e2e; array bounds raise `OutOfRangeException` |
| §4.5 calls: overloads, defaults, named args, function values, closures | ✅ | call/closure e2e; signature rules follow 1.162 (contracts affect type equality/mangling) |
| §4.6–4.7 construction/member construction | ✅ | construction tests; interface/union/variant elision included |
| §4.8 `is`: classes/interfaces/variants/float NaN | ✅ | RTTI and `is float(N).NotANumber`; statically impossible checks fold |
| §4.9 operators, derived comparisons, indexer, conversion, `??` | ✅ | operator/implicit conversion/coalescing tests |
| §4.9.3–4.9.29 unary/arithmetic/bitwise/shift/rotate/comparison/logical/conditional | ✅ | e2e arithmetic/shift/rotate/logical/conditional |
| §5.1 blocks/statement terminators | ✅ | parser/e2e |
| §5.2 inline assembly | ✅ | lexer/parser/placeholders/escapes/clobbers; operand-free blocks clobber all registers; operand blocks pin operands and clobber every other register on x86_64/i686/aarch64/armv7/riscv64; x86_64 runtime mov/add; `check` assembles asm-containing programs |
| §5.3–5.6 local/fixed/heap/linkable | ✅ | locals/fixed/heap, linkable functions and linkable variables/global external symbols (definitions or external declarations) |
| §5.7 use | ✅ | per-scope, later imports, multi-file tests |
| §5.8 assignment/compound assignment | ✅ | assignment/alias/write-through tests |
| §5.9 dispose | ✅ | heap units and field-pointer dispose tests; arbitrary pointer/container paths diagnosed per NOLAA conservatism |
| §5.10.1–5.10.5 control-flow names/if/while/do | ✅ | e2e control flow/labels/branch conditions |
| §5.10.6 for | ✅ | arrays/VLA/ranges/`&`/iterators; inferred and explicit binding types |
| §5.10.7 match | ✅ | patterns/guards/break/continue/labels/float special patterns |
| §5.10.8–5.10.9 break/continue | ✅ | labelled flow e2e |
| §5.11.1 exceptions, throw, trap | ✅ | construction/catch/finally/cleanup e2e |
| §5.11.2 return | ✅ | return typing and disposal ordering; value-type heap returns copy and dispose in callee |
| §6 names/scopes/modules/shadowing/accessibility | ✅ | scope/owner/use/accessibility tests |
| §7 modules | ✅ | module/nested-module tests |
| §8 functions/parameters/methods/overloads | ✅ | parameter/default/array-expansion/method/overload tests; `#`/`&` contracts participate in type equality and mangling |
| §9 modifiers (`scoped`, `readonly`, `implicitable`, `manual`, `abstract`, `sealed`, `virtual`, `override`, `aligned`, `static`, access) | ✅ | all enforced; duplicates diagnosed |
| §10 generics/monomorphization | ✅ | types/functions/constraints/extensions/caching/mangling; monomorphization base restricted to identifiers |
| §11.1–11.6 compile-time execution, `#` parameters, `#if` | ✅ | top-level/function-body blocks, expressions, order, constant parameters, conditionals |
| §11.7 conditional evaluation | ✅ | lazy two-phase settlement for immutable forward bindings; local/mutable forward deps rejected; no-progress errors; top-level global `fixed` constants are predeclared so `#if` can evaluate them |
| §11.8 compile-time boundary | ✅ | runtime reading of compile-time values, side-effect warnings, compile-time allocator lifetime |
| §11.9 compiler API: Type/Compiler/Environment/NOLAA | ✅ | `Type.SizeOf`, diagnostics API, Environment facts/queries (compile-time folding + runtime comparison for untracked facts; unprovable origins rejected), NOLAA markers |
| §12.1 memory allocation | ✅ | stack/heap/default/custom allocators, `manual`, value-type heap storage |
| §12.2 NOLAA | 🟡 | conservative by requirement: direct/pointer/field/element aliases, path-sensitive definitely/maybe disposal, parameter-disposal and read-only summaries, last-demand safe points, Bind/SCC/AssumeNoDemandNext, warnings; arbitrary points-to/unknown aliases stay conservative (allowed by §12.2) |
| §13 ABI | ✅ | layouts, calling conventions, mangling incl. `#`/`&` contract markers, RTTI/vtables |
| §14.1–14.2 main function/startup | ✅ | launcher/exit-code tests |
| §14.3.1 no-optimize markers | ✅ | `@(expr)`/`@{block}`/`local @`/`fixed @`, adjacency rules, optnone, literal materialisation |
| §14.3.2–14.3.4 optimisation permissions | ✅ | permissions not requirements; `-O` options |
| §15 Built-In Library | ✅ | Object/CallingConvention/PlatformKind/ArchitectureKind/EnvironmentKind/exceptions/Optional/Result/IIterator/IIterable/IAllocator/DefaultAllocator/compiler API; Environment module/facts |

## Known deviations / remaining backlog

1. **Compile-time non-scalar execution (§11.4)**: top-level blocks interpret
   scalar and scalar-field non-scalar heap objects, field reads/writes, dispose,
   `is` patterns/bindings, and `trap`/`throw`/`finally` including materialised
   thrown objects, catch bindings and class-hierarchy matching (when types are
   resolved). Remaining: constructor/destructor bodies of compile-time objects
   and non-scalar fields.

## Specification inconsistencies (not deliberate extensions)

- §2.7.12's function-literal EBNF allows only `Block | VerbatimBlock`, but the
  Draft 1.162 function-type-constructor example uses `(value: integer(4)) => value`.
  ai-nsc follows the example by accepting a single expression as an implicit
  return; this is a specification inconsistency, documented in
  `docs/spec-notes/_inconsistencies.md`, not a language extension.

## Capabilities and implementation notes

- NOLAA conservatism is required by §12.2, not a conformance deviation.
- EnvironmentFacts queries are compile-time folded when the frozen facts value is
  provable and rejected at compile time otherwise, per §11.9.3.
- UEFI target support and no-platform-allocator behavior are described in
  `docs/NEXT-SESSION.md`; they are platform coverage, not language rules.
