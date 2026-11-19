# Draft 1.153 conformance evidence

Authoritative specification: `docs/grammar/Draft 1.153.md`.

## Verification gates

```sh
export CARGO_HOME=/tmp/ainsc-cargo LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Current result: **420 tests green**, formatting clean, clippy with zero warnings.

Test breakdown:

| Target | Tests |
| --- | ---: |
| lib unit | 41 |
| CLI/driver | 140 |
| LLVM codegen | 2 |
| MCJIT probe | 1 |
| JIT compile-time evaluation | 2 |
| end-to-end execution | 221 |
| parser | 10 |
| spec-example conformance | 3 |

## Specification examples

`tests/spec_examples.rs` reads the specification directly:

- 97 `novasharp` fenced blocks are discovered;
- every block parses except five documented syntax fragments (not programs);
- every `Main`-containing example that is a complete program passes semantic
  analysis and compiles end to end;
- the exact set of `Main` examples with elided/demonstration-only bodies is
  pinned by `only_documented_spec_programs_fail_semantics`, so no new semantic
  failure can hide among skipped examples:
  `3802, 4748, 4816, 6581, 6650, 6713, 6892, 8869, 10508`.

## Draft 1.153 new-feature evidence

| Area | Evidence |
| --- | --- |
| §5.2 inline assembly | lexer/parser/escapes/placeholders/clobbers; operand-free blocks clobber all registers; operand blocks pin each operand and clobber every other register on x86_64/i686/aarch64/armv7/riscv64; x86_64 runtime mov/add |
| §11.9.3 Environment API | `EnvironmentFacts` structure/module, frozen Current/Host/Target values, field queries, facts-parameter/object-field/return-value specialization, runtime fallback field comparison; `#()` on unprovable facts rejected |
| §8.5/§13.2 `#`/`&` | function-type contracts participate in equality, spelling and mangling; regressions for overloads and name mangling |
| §9.5 `scoped` | duplicate `scoped` diagnoses; scoped targets resolved |
| §10.2 monomorphization | base must be an identifier; generic instantiation tests |
| §11.7 defaults | default expressions no longer required to be compile-time constants |

## Remaining deviation

`COVERAGE.md` lists one remaining deviation: compile-time constructor/destructor
bodies for non-scalar heap objects, non-scalar fields, and thrown object
values/catch bindings/class-hierarchy matching for compile-time `trap`.
Compile-time scalar-field heap objects, field reads/writes, `dispose` and `is`
patterns with named bindings are implemented. Other areas previously listed were
reclassified after auditing the specification: NOLAA conservatism is required by
§12.2, the expression-bodied function literal is a §2.7.9 EBNF/example
inconsistency, and unprovable EnvironmentFacts origins are conservatively
rejected at compile time per §11.9.3.

Audit history: `docs/AUDIT.md` (188 reproduced defects/fixes).
