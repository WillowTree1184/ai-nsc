# End-to-end tests

These tests drive the real `ai-nsc` executable and check observable behavior. The runner
will be a Rust integration test (or a thin script) added in Phase 2, because there is no
front end to drive yet.

## Planned format

```
tests/e2e/cases/<case-name>/
  main.nova           the program under test
  expected.out        exact expected stdout        (optional)
  expected.err        expected stderr, see below   (optional)
  exit_code           expected exit code           (optional, default 0)
  expected.ll         golden LLVM IR                (optional)
```

The runner will compare:

- `expected.out` byte-for-byte against stdout
- `expected.err` byte-for-byte, or as a regex when the file name is `expected.err.re`
- the process exit code (default 0)
- `expected.ll` through `FileCheck-18` against `ai-nsc emit-ir` output, so golden IR tests
  can use `// CHECK:` / `// CHECK-NEXT:` directives

Cases that must fail to compile live under `tests/e2e/cases/errors/` with a non-zero
`exit_code` and a matching `expected.err`.

See `docs/ROADMAP.md`.
