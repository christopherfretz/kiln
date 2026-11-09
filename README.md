<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo/kiln-banner-dark.svg">
    <img src="assets/logo/kiln-banner.svg" alt="kiln: C++26 LLVM frontend in Rust" width="480">
  </picture>
</p>

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

M6 (C++26 reflection and the reference workload) is closed and the M7
performance campaign is running; see [docs/ROADMAP.md](docs/ROADMAP.md) for
the milestones and [docs/BACKLOG.md](docs/BACKLOG.md) for work in progress. What kiln supports
in each standard, C++98 to C++26, feature by feature with the tests behind
each row, is in [docs/STANDARD_SUPPORT.md](docs/STANDARD_SUPPORT.md).

The libraries and applications kiln has been validated against (libc++,
libstdc++, and 47 open-source projects built through their own build
systems and test suites, including LLVM, Boost and folly), with each
result against a control build (clang 19), are in
[docs/COMPATIBILITY.md](docs/COMPATIBILITY.md).

## Layout

| Path | What |
|---|---|
| `crates/kiln_source`, `kiln_diag` | source management, diagnostics |
| `crates/kiln_lex`, `kiln_pp` | translation phases 1–4 |
| `crates/kiln_ast`, `kiln_parse`, `kiln_sema` | syntax and semantics |
| `crates/kiln_oven` | the constant evaluator |
| `crates/kiln_abi`, `kiln_llvm`, `kiln_codegen` | Itanium ABI, IR model, lowering |
| `crates/kiln_driver` | the `kiln` and `kiln++` commands |
| `xtask/` | test, bench and libc++ harness (`cargo xtask ...`) |
| `spec/` | per-crate contracts |
| `docs/` | architecture, roadmap, validation, contributor model, the standard support table (`STANDARD_SUPPORT.md`), the compatibility table (`COMPATIBILITY.md`) |
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
scripts/fetch-libcxx-tests.sh           # once: libc++ 19.1.7 tests into .toolchain/
cargo xtask libcxx --gate m3            # libc++'s own tests with kiln (manual gate, not in ci)
```

## Installing

Release archives for Linux on aarch64 and x86_64 are attached to each
[GitHub release](https://github.com/christopherfretz/kiln/releases):
`kiln-<version>-<triple>.tar.xz`, a `-symbols` archive with the full debug
information, and `SHA256SUMS`. Unpack one anywhere and put its `bin/` on
`PATH`:

```
sha256sum -c --ignore-missing SHA256SUMS
tar -xJf kiln-<version>-<triple>.tar.xz
export PATH=$PWD/kiln-<version>-<triple>/bin:$PATH
kiln++ -std=c++26 hello.cpp -o hello
```

The tree is relocatable: `bin/kiln`, `bin/kiln++`, the builtin headers and
predefines in `lib/kiln/<major>/`, and these documents in
`share/doc/kiln/` (layout and resource lookup:
[spec/driver.md](spec/driver.md) "Installation layout"). kiln writes LLVM
IR and needs an LLVM 19 (or 22) installation at run time for `llc`,
`llvm-as` and the `clang++` link step, plus the system's libstdc++ (or
libc++); see spec/driver.md "Installation layout" for the run-time
dependencies. `cargo xtask dist` builds the same archives from a checkout.

What a version number promises (driver flags, diagnostic ids, predefined
macros, ABI; not diagnostic wording or IR text) is in
[docs/STABILITY.md](docs/STABILITY.md).

## License

Dual-licensed under MIT or Apache-2.0, at your option. `resource/include`
is a copy of clang's builtin headers under the Apache-2.0 with LLVM
exceptions license; see `resource/include/LICENSE.TXT`.
