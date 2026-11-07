# libunifex with kiln++ (WP-789)

libunifex **v0.4.0** (tag `v0.4.0`, `17fecea`), Meta's sender/receiver
library, built with kiln++ as `CMAKE_CXX_COMPILER` through its own CMake
build (the `unifex` library, 62 tests and 32 examples) and its CTest
suite (94 entries; googletest 1.14 is fetched at configure time),
against a clang++-19 control build of the same tree, with the M8
harness (`scripts/oss/`, spec/landed/harness/WP-666.md). Host: aarch64
Linux, libstdc++ 14, C++20 (so `FindCoroutines` picks `<coroutine>` and
the coroutine paths are built), default `-fclang-compat`. Date:
2026-11-07.

**Result: parity.** Every TU compiles, every executable links and the
same 91 of 94 tests pass under both compilers. The three that fail
under both are the io_uring tests: `io_uring_setup` is not permitted in
the container (`std::system_error: Operation not permitted`).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh libunifex --compiler clang
scripts/oss/oss-build.sh libunifex
scripts/oss/oss-report.py libunifex
```

The `[libunifex]` row (`scripts/oss/projects.toml`) configures with no
extra options and tests with `ctest --output-on-failure --timeout 300
-j 4`. Each TU is compiled with `-O3 -DNDEBUG -std=gnu++20 -Wall -Wextra
-pedantic -Werror`. liburing 2.9 is found, so the io_uring context is
built too.

## Results

TUs are the harness's build-phase compiles (CMake probes excluded); wall
is the whole `oss-build.sh` run; Σ TU wall sums the wrapper's per-TU
wall times; peak TU RSS is the wrapper's `ru_maxrss`. One run each.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `8f9a969e4`) | 19 / 1 | 0 / 0 | 0/94 | 31 s | — | 150 MB |
| clang++-19 (control) | 114 / 0 | 94 / 0 | 91/94 | 194 s | 197.3 s | 542 MB (`async_scope_test.cpp`) |
| **kiln, final** (WP-789) | **114 / 0** | **94 / 0** | **91/94** | 278 s | 209.6 s | **223 MB** (`async_scope_test.cpp`) |

kiln's summed TU time is 1.06× clang's and its peak TU RSS 0.41×. The
first build stopped at the library (`source/task.cpp`), so nothing
linked. The final run shared the host with other harness builds, which
is most of the wall difference.

## Performance findings

None. No TU's peak RSS ratio reaches 1.2×. Seven TUs over 1 s took kiln
1.38-1.58× clang's time in the harness runs, which shared the host. The
two worst were re-timed back to back with GNU `time` (best of 3, the
same command with only the compiler swapped). `test/nest_test.cpp` was
1.24 s and 68 MB against 1.14 s and 184 MB (1.09×), and
`test/task_void_test.cpp` was 1.62 s and 75 MB against 1.47 s and
204 MB (1.10×).

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `sender_concepts.hpp:310: no matching function for call to object of type 'const _connect::_cpo::_fn'` (`source/task.cpp`: a `tag_invoke` friend template's signature needs its class's access) | 1 (blocks all) | fixed, `40c9ff5ae` | `class.friend/friend-template-signature-private-access-run.cpp` |
| 2 | `bind_back.hpp:115: redefinition of 'operator\|'` (a constrained friend naming a member class of the enclosing template) | 62 | fixed, `cc4ac95b5` | `temp.friend/constrained-friend-of-member-class-of-template-run.cpp` |
| 3 | `let_value_with_stop_token.hpp:243: invalid use of 'this' outside of a non-static member function` (a constructor's noexcept-specifier) | 3 | fixed, `550edbddb` (with `6cf9258ba`, `ad29f2a9f` behind it; after the merge, main's WP-792 fix of the same bug) | `except.spec/ctor-noexcept-implicit-this-member-run.cpp`, `temp.inst/constexpr-members-mutually-recursive-run.cpp`, `expr.prim.req.general/compound-type-constraint-pack-expansion.cpp` |
| 4 | `sync_wait.hpp:123: no matching function for call to object of type 'const _connect::_cpo::_fn'` (`type_erase_test`, `type_erased_stream`: a hard `protected destructor` error from an implicit destructor defined inside an alias substitution) | 2 | fixed, `b6a99bb70` | `temp.alias/implicit-dtor-defined-in-alias-checks-access-from-itself.cpp` |
| 5 | `when_all_2_test` timed out (miscompile: an NSDMI evaluated in `T{}` inside a member function used the enclosing `this`) | 1 test | fixed, `590b695a7` (after the merge, main's WP-792 fix of the same bug) | `dcl.init.aggr/default-member-init-this-is-the-aggregate-run.cpp` |

All repro paths are under `tests/conformance/`. No open kiln issue
remains in this project.

## What remains

- The three io_uring tests can only run where `io_uring_setup` is
  allowed; they fail identically under clang here.
- Variants not run through the harness: Debug, C++17 (the
  non-coroutine paths), sanitizers.
