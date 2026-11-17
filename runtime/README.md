# libnscrt — NovaSharp runtime support library

Generated code will call into a small C runtime instead of re-implementing
primitives in every program. This directory will hold its sources and the `libnscrt`
static library target, built with `clang` from `build.rs`.

Planned contents, pending the language spec:

- string representation and operations (length, compare, concatenate, format)
- array/list allocation and bounds checks
- printing/conversion primitives used by the language's I/O builtins
- panic/abort path and stack-trace hooks
- start-up code (`main` wrapper) if the language has top-level statements

Design constraints:

- C11, no C++ runtime dependency, no exceptions
- every entry point documented with its ABI (parameter and return conventions)
- allocation hooks route through one allocator so a future GC or arena can
  replace it without touching generated code

Nothing here is built yet; the directory is a placeholder so the layout is fixed
before codegen lands.
