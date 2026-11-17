# Toolchain

## Host

| Item | Value |
|---|---|
| OS | Ubuntu 24.04.5 LTS (noble), x86-64 |
| CPUs / RAM | 8 / 15 GiB |
| Rust | 1.98.0 (stable, rustup) |
| Cargo | 1.98.0 |
| rustfmt / clippy / rust-analyzer | 1.9.0 / 0.1.98 / 1.98.0 |
| LLVM (dev) | 18.1.3 — `llvm-config`, headers, `libLLVM-18.so` |
| Clang | 18.1.3 (used as assembler/linker driver) |
| inkwell / llvm-sys | 0.11.0 / 181.3.1, dynamic link (`llvm18-1-prefer-dynamic`) |
| Python | 3.12 |

LLVM 18 is the project baseline. Other LLVM majors (17, 20, 21) exist on this machine for
other projects, but only 18 has development files. If `llvm-config` on `PATH` is not 18,
point the build at the right installation:

```bash
LLVM_SYS_181_PREFIX=/usr/lib/llvm-18 cargo build
```

## Required (already installed)

`rustup` toolchain with `rustc`, `cargo`, `rustfmt`, `clippy`, `rust-analyzer`; `llvm-18-dev`
(provides `llvm-config` and `libLLVM-18.so`); `clang-18`; `lld`.

Verify with:

```bash
./scripts/check-toolchain.sh
```

## Optional

Present and useful: `llvm-18-tools` (`FileCheck-18` for IR golden tests), `valgrind` (memory
debugging), `lld`.

No longer needed after moving to Rust (installed earlier for a C++ scaffold, harmless to
keep or remove): `ccache`, `clang-format-18`, `clang-tidy-18`, `clangd-18`, `doxygen`,
`libgtest-dev`. Rust uses `rustfmt`/`clippy`/`rust-analyzer` instead.

## Build

```bash
cargo build                 # debug
cargo build --release       # optimized
cargo test                  # unit + integration tests
./scripts/lint.sh           # rustfmt --check + clippy -D warnings
```

Artifacts:

- `target/debug/ai-nsc`, `target/release/ai-nsc`
- `target/debug/ainsc` library used by integration tests

## Why the dynamic LLVM link

Ubuntu's `llvm-18-dev` ships the full set of static component libraries *except* some, and
inkwell's default static link pulls in components that are not available (e.g. Polly).
Selecting inkwell's `llvm18-1-prefer-dynamic` feature makes `llvm-sys` link the shared
`libLLVM-18.so`, which is complete and gives much faster link times.

## Editor setup

`rust-analyzer` works out of the box on a Cargo project; no `compile_commands.json` is
needed. `rust-toolchain.toml` pins the toolchain to stable with `rustfmt` and `clippy`
installed.

## Troubleshooting

- **`llvm-config` not found or wrong version** — install `llvm-18-dev`, or set
  `LLVM_SYS_181_PREFIX=/usr/lib/llvm-18` for cargo.
- **`libLLVM-18.so: cannot open shared object file`** — the binary expects LLVM's library
  directory in the loader path; use the system `llvm-18-dev` package (the build adds the
  library directory) and do not move the binary to a machine without LLVM 18.
- **cargo cannot write `~/.cargo`** — set `CARGO_HOME` to a writable directory. (Only
  relevant in sandboxed environments; a normal developer machine needs nothing.)
- **`inkwell` build errors mentioning `Polly`** — the dynamic-link feature above is
  missing; check `Cargo.toml` still requests `llvm18-1-prefer-dynamic`.
