# Spec coverage matrix

Status against `docs/grammar/Draft 1.12.md` (draft). Legend:

- ✅ implemented and covered by automated tests
- 🟡 implemented partially / with documented limitations
- ⛔ not implemented yet (diagnosed as "not implemented yet", never silently miscompiled)

Verification gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test` (99 tests: 36 unit, 8 CLI, 2 LLVM smoke, 42 end-to-end execution, 10 parser,
1 spec-example conformance), plus the CLI `ai-nsc check/emit-ir/run` smoke path.

| Spec area | Status | Notes / evidence |
|---|---|---|
| §2 lexical (tokens, comments, `@`, verbatim strings, encoding tags, `..`/`..!`) | ✅ | `src/lexer.rs` unit tests |
| Literal decoding (integer/float/character/string, encodings) | ✅ | `src/literal.rs` unit tests |
| §3.2 basic types, integer families, no implicit basic conversions | ✅ | e2e arithmetic/precedence tests |
| §3.5 value/reference semantics, class values are references | ✅ | e2e object model tests |
| §3.6.1 arrays (fixed dimensions), indexing, bounds check | ✅ | e2e arrays; bounds raise `OutOfRangeException` |
| §3.6.2 pointers | 🟡 | pointer types, member access, built-in pointer ops; `&`/`*` reference expressions pending |
| §3.6.3 function types / calling conventions | 🟡 | signatures parsed and checked; indirect calls through pointers pending |
| §3.7.1 classes: layout, inheritance, vtable/RTTI, construction, destructors | 🟡 | layout, construction, virtual dispatch, RTTI, `base`; statics pending |
| §3.7.2 structures: exact layout | ✅ | layout rules + tests |
| §3.7.3 interfaces: contracts, dispatch, RTTI | ✅ | e2e interface dispatch/RTTI tests |
| §3.7.4 enumerations | ✅ | member values, implicit-to-underlying conversion |
| §3.7.5 variants | ✅ | state/payload layout, member construction, `is` + match patterns |
| §3.7.6 unions | ✅ | overlap layout, `aligned(N)`, member construction/punning |
| §3.7.7 base list | 🟡 | one class + interfaces; generic targets pending |
| §3.8.1 fields, field initializers | ✅ | initialization order tests |
| §3.8.2 accessors | 🟡 | extension/interface accessors, backing get/set, block/expression forms; free/static accessors pending |
| §3.8.3 extensions | ✅ | methods, accessors, operators on types |
| §4.1–4.4 value/paren/cast/access expressions | ✅ | parser + e2e |
| §4.4.3 index access (array + indexer operator) | ✅ | bounds check; indexer get/set tests |
| §4.5 calls, overload resolution, default/named args | 🟡 | positional/default overload resolution; named args/function values pending |
| §4.6 construction, §4.7 member construction | ✅ | in-place construction tests |
| §4.8 `is` expressions (classes, interfaces, variants, special floats) | ✅ | RTTI e2e tests |
| §4.9 operators: overloads, derived comparisons, indexer, conversion | ✅ | operator e2e tests |
| §4.9.6/4.9.7 `&`/`*` reference expressions | ⛔ | diagnosed |
| §4.9.26 `??` | ⛔ | diagnosed (needs `Result`) |
| §4.9.27 `?:` conditional | ✅ | conditional expression e2e |
| §5 statements (blocks, locals, fixed, set/compound, if, while, do, for, match, break/continue, return, export) | ✅ | e2e statement tests |
| §5.10.3/4 `throw`, `trap`/`catch`/`finally` | 🟡 | source-level + runtime exceptions; `finally` on `return`/`break`/`continue` pending; heap-failure exceptions pending |
| §6 names/modules/`use`, scopes, shadowing | 🟡 | modules/scopes/`use` parsed and resolved; multiple input files pending |
| §7/§13/§14 ABI: name mangling, class layout, launcher, exit code | ✅ | mangling unit tests, LLVM smoke + e2e exit codes |
| §8 functions: parameters, defaults, overload resolution, `#` const params | 🟡 | defaults/overloads; `#` compile-time parameters and default-argument call lowering pending |
| §9 modifiers (accessibility, scoped, readonly, implicitable, verbatim, manual, abstract, sealed, virtual, override, aligned, static) | 🟡 | virtual/override/sealed/aligned/static parsing; accessibility and some modifier semantics not enforced |
| §10 generics / monomorphization | ⛔ | parser accepts syntax; semantic instantiation not implemented |
| §11.1–11.5 compile-time execution, `#(...)`, `#` params | ⛔ | diagnosed |
| §11.6 `#if` / `#else` | ✅ | constant evaluation, selected-branch-only checking/generation |
| §11.7 `#{...}` compile-time blocks | 🟡 | treated as blocks; compile-time variable constant propagation pending |
| §11.8 compiler API (`Type`, `Compiler`, `Platform`, NOLAA) | ⛔ | diagnosed |
| §12 memory model: locals, heap/dispose, NOLAA | 🟡 | heap/dispose; NOLAA analysis pending |
| §15 Built-In Library (Object, exceptions, Optional/Result, containers, iterators, compiler API) | 🟡 | exception class hierarchy only; rest pending |

Open draft inconsistencies and tolerated gaps are recorded in
`docs/spec-notes/_inconsistencies.md`; the conformance test parses all examples from
`docs/grammar/Draft 1.12.md` (one non-program type fragment skipped).
