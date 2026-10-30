# kiln

**A C++26 compiler frontend in safe Rust, built around a constexpr oven.**

kiln consumes C++26 source and emits LLVM IR; LLVM does optimization, code
generation and linking. Its reason to exist is compile-time evaluation:
constexpr, consteval and C++26 reflection workloads that make today's
compilers slow and memory-hungry. kiln's constant evaluator (the Oven) is
designed from the start to be the fastest and leanest available, with
wall-clock time and peak memory tracked as part of the test suite.

## Goals

- **Memory safe end to end.** Every crate is `#![forbid(unsafe_code)]`. No
  LLVM FFI: kiln writes LLVM IR itself and invokes LLVM tools as processes.
- **Production grade.** Fully conforming C++26, clang-compatible on the
  command line and in the predefined-macro/builtin surface so that libc++
  builds unchanged. clang is the oracle for differential testing.
- **The fastest constexpr evaluator anywhere.** Lazy bytecode compilation,
  a flat byte-addressed abstract memory model with shadow metadata (no
  per-value trees), per-evaluation arena reset, exact step/byte budgets,
  memoization, and reflection as a first-class VM value.
- **Specified and re-implementable.** Each crate has a contract in `spec/`
  tied to behavioral tests under `tests/`.

## Status

Early. See [docs/ROADMAP.md](docs/ROADMAP.md) for milestones (M1 is
preprocessor parity with clang on all of libc++'s headers) and
[docs/BACKLOG.md](docs/BACKLOG.md) for work in progress.

## Layout

| Path | What |
|---|---|
| `crates/kiln_source`, `kiln_diag` | source management, diagnostics |
| `crates/kiln_lex`, `kiln_pp` | translation phases 1–4 |
| `crates/kiln_ast`, `kiln_parse`, `kiln_sema` | syntax and semantics |
| `crates/kiln_oven` | the constant evaluator |
| `crates/kiln_abi`, `kiln_llvm`, `kiln_codegen` | Itanium ABI, IR model, lowering |
| `crates/kiln_driver` | the `kiln` command |
| `xtask/` | test, bench and libc++ harness (`cargo xtask ...`) |
| `spec/` | per-crate contracts |
| `docs/` | architecture, roadmap, validation, contributor model |
| `resource/` | clang 19 builtin headers and predefined macro sets |
| `tests/`, `bench/` | conformance/differential corpus, perf corpus |

## Building

Requires Rust (pinned in `rust-toolchain.toml`) and clang/LLVM 19 on PATH
(`clang++-19`, `llc`, `opt`, `lld`) for the backend and as the test oracle.

```
cargo build
cargo xtask ci                          # fmt, clippy, unit tests, corpus as clang self-check + kiln-mode (--quick skips the self-check)
cargo xtask test --compiler clang++-19  # validate the corpus against the oracle
cargo xtask test                        # run it through kiln
```

## License

Dual-licensed under MIT or Apache-2.0, at your option. `resource/include`
is a copy of clang's builtin headers under the Apache-2.0 with LLVM
exceptions license; see `resource/include/LICENSE.TXT`.
