# Final conformance audit (Beta 1.1 / Draft 1.162 semantics)

Date: 2026-11-19
Branch: `main`
Authoritative specification: `docs/LanguageSpecification/docs/specification/zh-cn.md`
(Git submodule `175f034`, branch `developing`). The remote text is titled
**NovaSharp Beta 1.1**; its diff against the previous local `Draft 1.162.md`
contains only the version title and callout formatting, not semantic changes.

## Gates

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | zero warnings |
| `cargo test` | **454 tests green** |
| Spec example parse test | **102/102** examples parse |
| Spec clean-program compile test | pass |
| Documented semantic-failure set | exact match (10 sources) |
| Stale `docs/grammar`/Draft path references | none |

Test breakdown: 41 lib unit, 150 CLI, 2 LLVM codegen, 1 MCJIT probe,
2 JIT compile-time evaluation, 245 end-to-end execution, 10 parser, 3 spec.

## 1.162-specific semantics

- Interface value constants: instance contracts, flattened inheritance, class
  public non-static `fixed` implementations, dispatch constant slots, runtime
  slot load, missing/static/private/type-mismatch diagnostics.
- Extension function constants: method-with-`this` or `static fixed` only;
  `static fixed` methods rejected.
- Field type may not be its declaring type; pointer self references remain valid.
- Bare `?? throw` accepts any `Result` Alternative payload; Optional explicit
  throw accepted.
- Compile-time objects:
  - constructors execute arguments, field writes, `this.Method()` calls, branches;
  - explicit `dispose` and end-of-block reverse destruction, exactly once;
  - constructor/destructor uncaught throws terminate with diagnostics;
  - pointer fields, pointer traversal, pointer cycles; class-reference fields;
  - nested value structures and array-valued fields;
  - copy-to-runtime: scalar structures/nested structures, one-dimensional
    scalar arrays, strings (byte arrays), enumerations, unions and variants with
    scalar payloads; pointer/class-bearing objects excluded per §11.8;
  - no NOLAA/automatic disposal for compile-time objects.
- `static fixed` type-level symbols/ABI and closure/function-value symbols.
- 1.16 migration coverage retained: inline assembly, `Environment`, `#&`,
  `scoped`, monomorphization, float special values, type-name elision.

## Remaining deviation

None. Compile-time constructor/destructor side effects on outer compile-time
variables are executed by the interpreter; methods carrying such side effects
are never emitted with those writes (their runtime construction/disposal is
diagnosed as compile-time-only, §11.4/§11.8).

## Remote

`origin` = `https://github.com/WillowTree1184/ai-nsc`; every completed round is
committed and pushed.
