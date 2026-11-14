<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo/kiln-banner-dark.svg">
    <img src="assets/logo/kiln-banner.svg" alt="kiln: C++26 LLVM frontend in Rust" width="480">
  </picture>
</p>

# kiln

**A C++26 compiler frontend in safe Rust, built around a constexpr oven.**

kiln compiles C++26 source, and C, to LLVM IR and leaves optimization,
code generation and linking to LLVM. It exists for compile-time evaluation:
constexpr, consteval and C++26 reflection code that makes compilers slow
and memory-hungry today. kiln's constant evaluator, the Oven, was designed
for that work from the start, and the test suite tracks its wall-clock
time and peak memory. Every crate is `#![forbid(unsafe_code)]`, and kiln
has no LLVM FFI: it writes IR as text and runs LLVM's tools. On the
command line and in its predefined macros kiln looks like clang, so build
systems and standard libraries work with it unchanged.

## Numbers

Each number links to the page that records how it was measured.

- **Constant evaluation.** On kiln's benchmark corpus, compared with
  clang 19 (37 entries, aarch64, 2026-11-03): geometric mean **0.21x**
  clang's time and **0.17x** its peak memory. Constexpr-bound entries such
  as `oven/fib` (0.05x) and `oven/churn-vector` (0.03x) gain the most.
  [docs/status/perf.md](docs/status/perf.md)
- **Constexpr-heavy libraries.** Built per TU against a control compiler
  (ctre, mp-units, daw_json_link, Glaze, scnlib, reflect-cpp): **0.39x to
  0.47x** the control's peak RSS (geomean per project), at 0.91x to 1.06x
  its time. [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md#constexpr-and-compile-time-performance)
- **libc++ 22.1.8** test suite (9247 tests, `-std=c++26`): **99.4%**
  pass. [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md#standard-libraries),
  [docs/status/libcxx-1.0.md](docs/status/libcxx-1.0.md)
- **47 open-source projects** built with their own build systems and test
  suites: 46 Flawless, 1 Near-perfect. They include all of LLVM
  (`check-llvm`: 38434 tests passed, at 0.94x clang's summed TU time and
  0.63x its peak TU RSS), Boost and folly.
  [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md#libraries-and-applications)
- **Language support:** 260 of 261 rows of the C++98 to C++26 paper table
  are done and 1 is partial.
  [docs/STANDARD_SUPPORT.md](docs/STANDARD_SUPPORT.md)

## Quick start

kiln runs on Linux, on aarch64 and x86_64. Release archives are attached
to each [GitHub release](https://github.com/christopherfretz/kiln/releases):
`kiln-<version>-<triple>.tar.xz`, a `-symbols` archive with full debug
information, and `SHA256SUMS`.

```sh
sha256sum -c --ignore-missing SHA256SUMS
tar -xJf kiln-<version>-<triple>.tar.xz
export PATH=$PWD/kiln-<version>-<triple>/bin:$PATH
```

The tree is relocatable: `bin/kiln`, `bin/kiln++`, the builtin headers
and predefines in `lib/kiln/<major>/`, and the documentation in
`share/doc/kiln/`.

kiln needs an LLVM installation with clang at run time, for `opt`, `llc`,
`llvm-as` and the `clang++` that links, plus libstdc++ (or libc++).
LLVM 22 (preferred) and LLVM 19 are supported. kiln finds an installation
in the usual places on its own. `--llvm-dir=<dir>` or `KILN_LLVM_DIR`
names one, and `kiln++ --version` prints the choice. On Debian 13:

```sh
sudo apt install clang-19 llvm-19 g++    # LLVM 19 from Debian's archive, and libstdc++
```

LLVM 22 comes from [apt.llvm.org](https://apt.llvm.org). The packages
for Ubuntu and Fedora are in
[docs/COMPATIBILITY.md](docs/COMPATIBILITY.md#runtime-dependencies).

Then compile:

```sh
kiln++ -std=c++26 hello.cpp -o hello && ./hello
kiln -std=c17 hello.c -o hello && ./hello
CC=kiln CXX=kiln++ cmake -S . -B build && cmake --build build
```

`kiln++` is the C++ driver and `kiln` the C driver, as `clang++` and
`clang` are.

CMake identifies kiln as Clang and needs no changes. Autotools works
unchanged too. Meson needs a wrapper. The
[user guide](docs/USER_GUIDE.md) covers installation, build systems,
language modes, Oven limits and flags.

## Building from source

The Rust toolchain is pinned in `rust-toolchain.toml` (Rust 1.95.0, which
is also the MSRV). The backend and the test oracles need clang and LLVM 22
and 19 (`/usr/lib/llvm-22` and `/usr/lib/llvm-19`, with `clang++-22` and
`clang++-19` on `PATH`), which is what CI installs.

```sh
cargo build --release              # target/release/kiln and kiln++
cargo xtask ci --quick             # fmt, clippy, unit tests, the corpus through kiln
cargo xtask dist                   # release archives, as published, from this checkout
```

`cargo xtask dist --check` also extracts the archive and smoke-tests it.
The archive layout is in [spec/driver.md](spec/driver.md) under
"Installation layout".

### Profile-guided build

```sh
cargo xtask build --pgo            # target/release/kiln++, profile-guided
cargo xtask dist --pgo             # the release archives, profile-guided
```

`--pgo` builds kiln instrumented, has it compile a fixed training corpus
from `bench/`, merges the profile with `llvm-profdata` and builds again
with it. It takes about three times as long as a plain build and gives a
faster binary with the same memory use (see
[docs/status/perf.md](docs/status/perf.md), "PGO build").
`llvm-profdata` must be the same LLVM major as rustc's (22 for Rust
1.95). It is found in the LLVM that kiln uses, or `--llvm-dir=<dir>`
names one, or it comes from rustup's `llvm-tools` component. Without
`--pgo` nothing changes. The contract is in
[spec/harness.md](spec/harness.md) under "PGO build".

**For packagers.** If your own build runs `cargo build --release`, run
the pre-step first, then build as usual with the profile in the
rustflags:

```sh
cargo xtask pgo-profile --out "$PWD/kiln.profdata"   # instrumented build, training, merge
RUSTFLAGS="$RUSTFLAGS -Cprofile-use=$PWD/kiln.profdata" cargo build --release
```

Give the pre-step the same `RUSTFLAGS` (or `CARGO_ENCODED_RUSTFLAGS` /
`CARGO_BUILD_RUSTFLAGS`) as the final build. It drops any
`-Cprofile-use` it finds there. Use the same rustc, source tree,
`--target` argument (pass `--target` to `pgo-profile` if the final build
passes one) and `--features`. If any of these differ, some or all
functions do not match the profile. rustc does not fail on that, it just
builds without the profile. To check that the profile applied, add
`-Cllvm-args=-pgo-warn-missing-function` to the final build's flags.
rustc then warns once for each function without profile data. With a
matching profile the only warnings are for functions in
`build_script_build`, which the training never runs. A mismatched
profile warns for nearly every function of kiln's crates. The `-Cprofile-use` path must be
absolute. The final build can use any target directory.

The steps never use the network. The pre-step builds `xtask`, so the
vendored crates must include its dependencies (`signal-hook`,
`signal-hook-registry`, `libc`, `errno`, all in `Cargo.lock`). It also
needs the LLVM installation (with `llvm-profdata`, `clang++` and the
libc++ headers) and the system libstdc++. GCC 16.1 (`KILN_GCC`) is
optional. Without it, five reflection cases are left out of the training
with a warning. To build the instrumented binary another way, `cargo
xtask pgo-train --compiler <kiln++> --profile-dir <dir>` runs the
training alone and prints the `llvm-profdata merge` command.

Here is a `debian/rules` sketch. It has not been tried with a Debian
Rust helper. `KILN_RUSTFLAGS` and `--target` stand for whatever your
helper's `cargo build` really uses, which its verbose output shows. A
helper that adds `--target` or `-C debuginfo` to its own build has to
get the same from the pre-step:

```make
include /usr/share/rustc/architecture.mk      # DEB_HOST_RUST_TYPE
PROFDATA := $(CURDIR)/debian/kiln.profdata

override_dh_auto_build:
	RUSTFLAGS="$(KILN_RUSTFLAGS)" cargo xtask pgo-profile \
	    --target $(DEB_HOST_RUST_TYPE) --out $(PROFDATA)
	RUSTFLAGS="$(KILN_RUSTFLAGS) -Cprofile-use=$(PROFDATA)" dh_auto_build
```

## Status

kiln is at version 1.2.0; what changed is in
[CHANGELOG.md](CHANGELOG.md), and what a version promises in
[docs/STABILITY.md](docs/STABILITY.md). The current milestone, M8, builds real open-source projects with kiln. The
milestones are in [docs/ROADMAP.md](docs/ROADMAP.md).

| Area | State | Detail |
|---|---|---|
| C++98 to C++23 | done | [STANDARD_SUPPORT.md](docs/STANDARD_SUPPORT.md) |
| C++26 (Croydon DIS draft), reflection, contracts | done except 1 partial paper (P2786, removed from the DIS) | [STANDARD_SUPPORT.md](docs/STANDARD_SUPPORT.md) |
| libc++ 22 / libc++ 19 / libstdc++ 14 | 99.4% / 98.3% / every header | [COMPATIBILITY.md](docs/COMPATIBILITY.md#standard-libraries) |
| Debug info, sanitizers, gcov and source coverage | supported; gaps listed | [KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md) |
| Modules | supported, with module files and CMake dependency scanning; no header-unit module files yet; the declaration image (`-fkiln-bmi-image`) is experimental and off by default | [USER_GUIDE.md](docs/USER_GUIDE.md#modules) |
| Precompiled headers | supported (C++ and C); since 1.2.0 a consumer restores a snapshot of the header's state (`<bits/stdc++.h>`: 0.57 s against 0.67 s for clang 19's PCH) and replays the header when it cannot; `-fno-kiln-pch-snapshot` turns it off | [USER_GUIDE.md](docs/USER_GUIDE.md#precompiled-headers), [spec/pch.md](spec/pch.md), [KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md#precompiled-headers) |
| C89 to C23 and GNU C (`kiln`) | supported since 1.1.0; gaps listed | [KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md#c-mode), [status/c-mode.md](docs/status/c-mode.md) |
| Platforms | Linux aarch64, Linux x86_64 | [KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md#platforms) |

## Documentation

For users:

- [docs/USER_GUIDE.md](docs/USER_GUIDE.md): installing, build systems,
  language modes, the Oven, flags, environment variables, debugging.
- [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md): what does not work yet.
- [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md): every library and
  project kiln was tested against, with results.
- [docs/STANDARD_SUPPORT.md](docs/STANDARD_SUPPORT.md): the language
  features, paper by paper.
- [docs/STABILITY.md](docs/STABILITY.md): what a version number promises.
- [docs/REPORTING-BUGS.md](docs/REPORTING-BUGS.md): crash reports and
  reproducers. Security issues: [SECURITY.md](SECURITY.md).
- [CHANGELOG.md](CHANGELOG.md).

For contributors:

- [CONTRIBUTING.md](CONTRIBUTING.md): building, testing and sending
  changes.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): the crate pipeline and the
  Oven's design.
- [spec/](spec/): one contract per crate.
- [docs/ROADMAP.md](docs/ROADMAP.md),
  [docs/BACKLOG.md](docs/BACKLOG.md): milestones and work packages.

## Layout

| Path | What |
|---|---|
| `crates/kiln_source`, `kiln_diag` | source management, diagnostics |
| `crates/kiln_lex`, `kiln_pp` | translation phases 1–4 |
| `crates/kiln_ast`, `kiln_parse`, `kiln_sema` | syntax and semantics |
| `crates/kiln_oven` | the constant evaluator |
| `crates/kiln_abi`, `kiln_llvm`, `kiln_codegen` | Itanium ABI, IR model, lowering |
| `crates/kiln_driver` | the `kiln` and `kiln++` commands |
| `xtask/` | test, bench, libc++, OSS and release harness (`cargo xtask ...`) |
| `spec/` | per-crate contracts |
| `docs/` | user and contributor documentation, status pages |
| `resource/` | clang's builtin headers and predefined macro sets |
| `tests/`, `bench/` | conformance and differential corpus, perf corpus |

## Development

kiln is written by agents working on work packages, coordinated by a
supervisor. Their contract is [docs/AGENTS.md](docs/AGENTS.md). The test
layers, the test directive format and perf tracking are described in
[docs/VALIDATION.md](docs/VALIDATION.md). Human contributors start with
[CONTRIBUTING.md](CONTRIBUTING.md).

## Reporting bugs

File issues at https://github.com/christopherfretz/kiln/issues. When kiln
crashes it prints an "internal compiler error" report and writes a
reproducer: a preprocessed `.ii` and a `.sh` that reruns the command.
Attach both. See [docs/REPORTING-BUGS.md](docs/REPORTING-BUGS.md).

## License

Dual-licensed under MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](LICENSE-APACHE)), at your option. `resource/include` is
a copy of clang's builtin headers under the Apache-2.0 with LLVM
exceptions license; see `resource/include/LICENSE.TXT`.
