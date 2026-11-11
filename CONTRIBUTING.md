# Contributing to kiln

Thanks for your interest. This page covers building kiln, running the
tests, and getting a change merged. Most work in this repository is done
by agents under the contract in [docs/AGENTS.md](docs/AGENTS.md). The
same rules apply to human contributors.

## Build

You need:

- **Rust 1.95.0.** `rust-toolchain.toml` pins it, so `rustup` picks it
  up. 1.95 is also the MSRV: do not use std APIs, language features or
  clippy lint names stabilized after 1.95.
- **clang and LLVM 19 and 22** (`clang++-19`, `clang++-22`, `llc`, `opt`,
  `llvm-as`, `lld`) and libc++ 19. LLVM is kiln's backend, and clang is
  the oracle the corpus is checked against. On Debian 13 this is what CI
  installs (`.github/actions/kiln-ci/action.yml`):

  ```sh
  apt install build-essential clang-19 llvm-19 lld-19 libc++-19-dev libc++abi-19-dev \
              clang-22 llvm-22 lld-22 gdb lldb-19
  ```

  `clang-22` comes from [apt.llvm.org](https://apt.llvm.org/). GCC 16.1
  (`KILN_GCC`) is the reflection and contracts oracle. Tests that need it
  are skipped without it.

```sh
cargo build                 # target/debug/kiln and kiln++
cargo build --release
cargo xtask dist --check    # the release archives, smoke-tested
cargo xtask build --pgo     # profile-guided release build, ~3x the build time (README.md)
```

## Test

```sh
cargo xtask ci --quick                     # what to run before sending a change
cargo xtask test tests/conformance/<dir>   # one corpus directory, through kiln
cargo xtask test --compiler clang++-22 tests/conformance/<dir>   # the same against clang
cargo test -p <crate>                      # one crate's unit tests
```

`cargo xtask ci --quick` runs the lint checks, `cargo fmt --check`,
`cargo clippy -D warnings`, the unit tests and the corpus through kiln.
The full `cargo xtask ci`, which adds the clang self-check, runs in the
merge queue.

A corpus test is a single source file whose comments carry directives:
the expected result, diagnostics, run output, standard section and
clang/GCC oracle. The format is in
[docs/VALIDATION.md](docs/VALIDATION.md#conformance-test-format) and
[spec/harness.md](spec/harness.md). A known kiln gap is marked
`xfail(kiln): <reason>` in its test, not left failing.

Every new behaviour ships with a test that would fail without it, and
the test names its standard section. Performance-sensitive changes also
ship with a benchmark baseline (`bench/`,
[docs/VALIDATION.md](docs/VALIDATION.md#performance-tracking)).

## Spec first

Each crate has a contract in [spec/](spec/), and the code implements the
spec. A change in behaviour changes the spec in the same pull request,
together with its tests. A work package that lands behaviour records it
in its own file, `spec/landed/<spec>/WP-NNN.md`. `cargo xtask ci` checks
that the file has a matching row in [docs/BACKLOG.md](docs/BACKLOG.md).
See [spec/README.md](spec/README.md).

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and the spec of the
crate you are changing before writing code.

## Rules

- **No `unsafe`.** Every crate is `#![forbid(unsafe_code)]`, and that
  stays. kiln has no LLVM FFI. It writes IR as text and runs LLVM's tools
  as processes.
- **Diagnostics, not panics.** Anything that user input can cause is a
  diagnostic. Internal invariants use `unreachable!` or `debug_assert!`
  with a message.
- **No new dependencies** without agreement in the issue or work package
  first.
- **Diagnostic ids** go in kiln_diag's area files, sorted by name
  ([spec/diag.md](spec/diag.md) §2.4; `cargo xtask diag-split`).
- **Public interfaces** between crates do not change without a spec
  update, because other work may be building against them.

## Filing bugs

Use [GitHub issues](https://github.com/christopherfretz/kiln/issues). For
a crash, attach the `.ii` and `.sh` reproducer kiln writes. For anything
else, attach a reduced test case and the command line.
[docs/REPORTING-BUGS.md](docs/REPORTING-BUGS.md) covers both. Security
issues go through [SECURITY.md](SECURITY.md), not public issues.

## Pull requests

- Branch from `main`. Keep a change to one topic: one work package, one
  spec section.
- Before opening the pull request, merge the latest `main` and make
  `cargo xtask ci --quick`, fmt and clippy pass.
- The description says what changed, which spec sections and tests
  cover it, and any interface change.
- A maintainer reviews the pull request and adds it to GitHub's merge
  queue. The queue runs the full `cargo xtask ci` on aarch64 and x86_64
  against the latest `main` and merges only if it passes
  ([docs/VALIDATION.md](docs/VALIDATION.md#github-actions)). Nobody
  pushes to `main` directly.

## License

By contributing, you agree that your contributions are dual-licensed
under MIT and Apache-2.0, as the rest of kiln is.
