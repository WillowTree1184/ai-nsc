# Draft 1.16 conformance evidence

Authoritative specification: `docs/grammar/Draft 1.16.md`.

## Verification gates

```sh
export CARGO_HOME=/tmp/ainsc-cargo LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Current result: **424 tests green**, formatting clean, clippy with zero warnings.

| Target | Tests |
| --- | ---: |
| lib unit | 41 |
| CLI/driver | 140 |
| LLVM codegen | 2 |
| MCJIT probe | 1 |
| JIT compile-time evaluation | 2 |
| end-to-end execution | 225 |
| parser | 10 |
| spec-example conformance | 3 |

## Specification examples

`tests/spec_examples.rs` reads Draft 1.16 directly:

- 102 `novasharp` fenced blocks are discovered;
- all parse except five documented non-program syntax fragments;
- every complete `Main` program passes semantic analysis and compiles end to end;
- demonstration-only `Main` examples with elided bodies are pinned:
  `3858, 4834, 4902, 6692, 6761, 6824, 7003, 8983, 10620`, so a new semantic
  failure cannot hide among skipped examples.

## Draft 1.16 new-feature evidence

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

## Remaining deviations

See `docs/COVERAGE.md` for the current five real gaps: compile-time
constructor/destructor/thrown-object semantics, function-field `Block`
initializers, `linkable` variables, `#if` on global `fixed` constants, and
non-function interface constant contracts. NOLAA conservatism is required by
§12.2 and is not a deviation.

Audit history: `docs/AUDIT.md` (188 reproduced defects/fixes).
