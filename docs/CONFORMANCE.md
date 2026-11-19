# Beta 1.1 conformance evidence

Authoritative specification: `docs/LanguageSpecification/docs/specification/zh-cn.md`.

## Verification gates

```sh
export CARGO_HOME=/tmp/ainsc-cargo LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Current result: **443 tests green**, formatting clean, clippy with zero warnings.

| Target | Tests |
| --- | ---: |
| lib unit | 41 |
| CLI/driver | 146 |
| LLVM codegen | 2 |
| MCJIT probe | 1 |
| JIT compile-time evaluation | 2 |
| end-to-end execution | 238 |
| parser | 10 |
| spec-example conformance | 3 |

## Specification examples

`tests/spec_examples.rs` reads Beta 1.1 directly:

- 102 `novasharp` fenced blocks are discovered;
- all parse except five documented non-program syntax fragments;
- every complete `Main` program passes semantic analysis and compiles end to end;
- demonstration-only `Main` examples with elided bodies are pinned:
  `3858, 4834, 4902, 6692, 6761, 6824, 7003, 8983, 10620`, so a new semantic
  failure cannot hide among skipped examples.

## Beta 1.1 new-feature evidence

| Area | Evidence |
| --- | --- |
| §3.3 float special values | `float(N).NotANumber/Infinite`, negative infinity, `is float(N).NotANumber` (NaN compare), NaN equality, match patterns; old `illegal`/`infinite` removed from lexer/parser |
| §3.8.3 direct interface contracts | interface bodies declare constants/accessors/operators directly, class-body extensions implement them; e2e interface dispatch/RTTI/inheritance |
| implicit `this` fields | `Resolution::ThisField` for bare field reads/writes/lvalue/accessor backing in methods |
| §3.8.5 type-name elision | enum members, union member constructions and variant states in typed contexts |
| global constants | `fixed Flag = true` inferred from initializer |
| §5.2 inline assembly | operand-free all-register clobber; operand blocks pin operands and clobber every other target register (x86_64/i686/aarch64/armv7/riscv64 object emission; x86_64 runtime) |
| §11.9.3 Environment API | EnvironmentFacts module/structure/values, field/parameter/object-field/return specialization, runtime facts comparisons, compile-time rejection of unprovable origins |
| §8.5/§13.2 `#`/`&` | contracts participate in type equality and mangling; only-contract overloads rejected |
| §9.5/§10.2/§11.7 | duplicate `scoped` rejected; identifier-only monomorphization base; default expressions are caller-side runtime values |

## Non-`Main` example diagnostics

A full `ai-nsc check` scan over all 102 fences reports 32 diagnostics. Most are
expected demonstration/fragment material: elided function bodies, bare type or
closure syntax, undefined illustrative names, the launcher's multiple `Main`
example, and the complete Built-In Library source that necessarily redeclares
the compiler's built-ins. Real implementation gaps still visible in that scan
are tracked in `COVERAGE.md` (currently the two deviations below) plus
function-valued extension constants and some accessor-backing/`this`-scope
examples; these are not hidden by the `Main`-program conformance test.

## Remaining deviations

See `docs/COVERAGE.md` for the current real gap: compile-time
constructor/destructor exceptions/arbitrary side effects, non-scalar/pointer
fields, and union/variant copy-to-runtime coverage. NOLAA conservatism is required by §12.2 and is not a
deviation.

Audit history: `docs/AUDIT.md` (199 reproduced defects/fixes).
