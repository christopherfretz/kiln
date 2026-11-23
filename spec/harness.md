# Harness: `cargo xtask test`

Status: implemented (WP-003). Code: `xtask/src/test/`. The directive format is
introduced in `docs/VALIDATION.md`; this file is the normative contract for the
runner. It is the contract a re-implementation of the runner must satisfy, and
the contract a test author can rely on.

## Command line

```
cargo xtask test [options] [paths...]
```

| Option | Meaning |
|---|---|
| `--compiler <path>` | Compiler under test. Default: `cargo build -p kiln_driver`, then `kiln++` (the `kiln` binary under its C++ name, made beside it if missing) in the target dir (`$CARGO_TARGET_DIR`, else `target_directory` from `cargo metadata`), `debug` profile. |
| `--oracle <path>` | Oracle compiler for `run`/`pp`/`ast`/`layout`/`vtables`/`ir` comparisons. Default `clang++-N` of `KILN_ORACLE` (`clang++-22`; see "Oracle toolchain"). |
| `--no-oracle` | Skip all oracle comparisons, clang's and GCC's (`layout`, `vtables` and `ir` tests, which have no other reference, are then a SKIP). |
| `--gcc <path>` | The GCC oracle of `oracle: gcc` tests (see "GCC oracle"). Default `$KILN_GCC`, else `g++-16` on `PATH`. A GCC named by the flag or by `KILN_GCC` that does not run or does not identify as GCC is a usage error (exit 2); the default one missing drops the GCC comparisons. |
| `--no-gcc` | Drop the GCC comparisons only. |
| `--meta-header` | The header-differential mode: the reflection tests compile against GCC 16.1's real `<meta>` instead of kiln's `tests/support/kiln_meta.h` (see "Header-differential mode"). Needs kiln as the compiler under test and a GCC oracle whose install prefix can be found (a usage error, exit 2, otherwise). |
| `--debug-sweep` | The debug-info sweep: compile each positive test with and without `-g` instead of checking its expectations (see "Debug-info sweep"). Not with `--meta-header` (usage error). |
| `--bmi-kind 1\|2` | The module-file kind variant: only the tests with `module:` or `header-unit:` lines, every one two-step, with kiln writing kind-`N` module files (see "Module-file kinds"). Needs kiln as the compiler (otherwise a usage error). WP-985. |
| `--bmi-image` | With `--bmi-kind 2` (implied when no kind is given; with `--bmi-kind 1` a usage error): kiln's module-file builds and its importing compiles also get `-fkiln-bmi-image`, so units that qualify are imported through the declaration image (see "Module-file kinds"). WP-1048. |
| `--pch-diff replay\|snapshot` | The PCH differential: each test that opens with `#include`s is compiled again with a PCH of them, and the two results are compared instead of the test's expectations (see "PCH differential"). Needs kiln as the compiler (otherwise a usage error); no oracle runs; not with another variant (usage error). WP-1036; `snapshot` WP-1383/WP-1395; `load` and `check` retired with the declaration image (WP-1397). |
| `--pch-diff-require-restore` | With `--pch-diff snapshot` (else a usage error): a test whose consumer replayed the header for a reason `tests/pch-snapshot-replay-allow.txt` does not allow FAILs (see "PCH differential"). WP-1395. |
| `--pch-diff-extra-dir` | With `--pch-diff snapshot` (else a usage error): every other test's consumer (by a hash of its name) also gets `-I` of an empty directory, so its search list differs from the producer's; it must still restore, and must say it repeated the snapshot's lookups, or the test FAILs (see "PCH differential"). WP-1403. |
| `--pch-diff-synthetic` | With `--pch-diff` (else a usage error): a test with no `#include` line gets the synthetic prefix `#include <bits/stdc++.h>` (see "PCH differential"). WP-1395. |
| `--external <dir>` | Instead of the corpus, the never-crash run of a directory outside the repository (see "External corpora"). |
| `--filter <substr>` | Keep only tests whose name (repo-relative path) contains `<substr>`. |
| `--shard K/N` | Run only shard `K` of `N` (`1 <= K <= N`, else a usage error): of the sorted test list (after `--filter`), test `i` (0-based) runs iff `i % N == K - 1`. Round-robin, so a slow directory is spread over every shard, and shards `1..N` of one `N` are exactly the unsharded run, each test in one shard. A shard of a list shorter than `N` may be empty, which is not an error (an empty selection before sharding still is). The run's first line says `(shard K/N of M)`. Not with `--external` (usage error). `ci --shard` passes it to both corpus runs (WP-692, "CI"). |
| `--update` | Rewrite existing pp and ast `.expected` goldens with the actual output. |
| `--pp-compare tokens\|text` | Override every pp test's `pp-compare:` directive (see "pp comparison"). |
| `--report-parity <file>` | After the run, write a parity table into `<file>` (see "Parity report"). A relative path is relative to the repo root. |
| `-j, --jobs <N>` | Parallel tests. Default `min(available CPUs, 8)`, capped at the test count. Under an inherited jobserver an upper bound (see "Jobserver"). |
| `--timeout <secs>` | Run-time limit for test binaries (float seconds, default 10). Compiles have a fixed 120 s limit. Every process runs in its own process group, and a timeout kills the group, so a compiler's subprocess (clang's `cc1`) cannot outlive it. `cargo xtask libcxx` takes the same option for its test binaries (default 30 s; its compile limit is `--compile-timeout`, 120 s, 300 s with `--profile debug`, `spec/libcxx.md` "Build profiles"); a long libc++ test, such as `rand.dist.bern.negbin/eval.pass.cpp` (about 31 s even as clang's `-O0` binary, `REQUIRES: long_tests`), gets `--run-timeout-long` instead (default 240 s, `spec/libcxx.md` "Timeouts and retries"). |
| `--mem-limit <MiB>` | Address-space cap of every process a test runs: compiler, oracle and the built binaries. Default 8192; `0` disables the default cap (a test's `mem<=N` still applies). See "Resource limits". |
| `-v, --verbose` | Print every result and every command that was run. |
| `-h, --help` | Usage. |

`--flag=value` is accepted for every option that takes a value.

`paths` are files or directories (default: `tests/c`, `tests/conformance`,
`tests/debuginfo`, `tests/differential` and `tests/stress`, whichever
exist; `tests/debuginfo` is a default root everywhere, its tests SKIP
without gdb and lldb-19, see "Debugger tests").

Exit status: **0** when there is no FAIL, XPASS, ERROR or KILLED (XFAIL and
SKIP are allowed); **1** when there is at least one; **2** for a usage error, an
infrastructure error (kiln build failed, compiler not runnable, scratch dir not
creatable), or an empty test selection.

## Discovery

- Directories are walked recursively. Files and directories whose names start
  with `.` are ignored, and so are directories named `Inputs` (the lit
  convention for auxiliary files), including an `Inputs` directory named on
  the command line (WP-917).
- `.cpp` and `.c` files are tests. A `.h` file is a test only if its header
  contains a `//@` directive; otherwise it is an include file.
- A file named explicitly on the command line is always a test.
- `tests/stress/` holds the tests that push a resource: deep nesting, huge
  packs, the stack guard, and regressions of runaway allocation. They are
  ordinary tests (any `expect:`), usually with a `mem<=N` budget.
- The test list is sorted by name and de-duplicated. A test's name is its path
  relative to the repo root (or absolute, if it lies outside the root).

## Directives

The directive header is the leading run of lines that are blank or start with
`//`. Directives are the lines in it that start with `//@`, written as
`//@ key: value`. A `//@` line after the header is an ERROR, because it would
otherwise be silently ignored. Unknown keys, malformed values and conflicting
directives are also ERRORs.

| Directive | Semantics |
|---|---|
| `expect: compile-ok \| compile-fail \| run \| pp \| ast \| cfg \| layout \| vtables \| ir \| debugger \| no-crash \| ice` | Test kind. Default `compile-ok`. |
| `std: <std>` | `c++98 c++03 c++11 c++14 c++17 c++20 c++23 c++26` (C++), `c89 c99 c11 c17 c23` (C). Default `c++26` / `c17`. Passed as `-std=<std>`. |
| `args: <flags>` | Extra compiler flags, split shell-style (quotes, backslash). It may be repeated, and the flags accumulate. Placeholders are substituted in each flag (below). |
| `diag: <line>[:<col>] <sev> "<substring>" [dup(clang)]` | Expected diagnostic. `<sev>` is one of `error fatal warning note remark`. If the column is omitted, any column matches. `dup(clang)`, after the substring and only on a primary (non-note) directive, says clang prints this group twice where kiln's engine prints it once (spec/diag.md §7.4.1); see "Diagnostics". |
| `strict-diags` | The `diag:` list is exhaustive for diagnostics located in the test file. |
| `verify` | `compile-ok` only; takes no value. The test's expectations are clang `expected-*` comments (spec/diag.md §11), checked by the compiler's own `-Xclang -verify` and by the oracle's (see "verify tests"). It cannot be combined with `diag:` or `strict-diags`. |
| `stdout: <text>` | `run` only. Each line contributes its value, trimmed, plus `\n`. Repeated lines concatenate in order. |
| `stdout-file: <path>` | `run` only. The expected stdout is the file's exact contents. The path is relative to the test's directory. It cannot be combined with `stdout:`. |
| `exit: <code>` | `run` only. Expected exit code. Default 0. |
| `gdb: <command>` / `lldb: <command>` | `debugger` only. One line of the gdb (lldb) command script, in order; repeatable. See "Debugger tests". |
| `ice: <text>` | `ice` only. A substring the crash report's stderr must contain, each after the end of the previous one's match (so two may sit on one line); repeatable. `diag:` and `strict-diags` do not apply to `ice` tests (an ERROR). See "ice tests". |
| `gdb-check: <text>` / `lldb-check: <text>` | `debugger` only. A substring that must appear in the debugger's output, each check on a line after the previous check's line; repeatable. Commands without checks, checks without commands, and a `debugger` test with neither debugger are ERRORs. |
| `run-args: <args>` | `run` only. Program arguments, split shell-style as for `args:`. It may be repeated, and the arguments accumulate. |
| `run-stdin: <text>` | `run` only. Each line contributes its value, trimmed, plus `\n`, to the program's stdin, as for `stdout:`. Repeated lines concatenate in order. |
| `run-stdin-file: <path>` | `run` only. The program's stdin is the file's exact contents. The path is relative to the test's directory (use `Inputs/`, which discovery skips). It cannot be combined with `run-stdin:`. A missing file is an ERROR. |
| `env: <NAME>=<value>` | `run` only. Sets an environment variable for the program (the inherited environment is otherwise kept). `NAME` matches `[A-Za-z_][A-Za-z0-9_]*`; the value is the rest of the line, trimmed, and may be empty or contain `=`. It may be repeated, once per name. |
| `oracle: clang \| gcc \| none` | `run`/`pp`/`ast`/`verify` only (`gcc`: `run`, `compile-ok` and `compile-fail` only, not `verify`). Whether to compare with the oracle, and which: `gcc` compares with the GCC oracle instead of clang (see "GCC oracle"). Defaults to `clang` for `run`, `pp` and `verify`, `none` for `ast` (the golden is the contract). `layout`, `vtables` and `ir` tests are always compared with the oracle (they have no golden), so the directive is an error there. |
| `cfg-diverges: <function> <reason>` | `cfg` only. The graphs of `<function>` (matched as in "cfg tests") are left out of the oracle comparison, because clang's analyzer dump differs from its warnings' graph or from kiln's by construction; `<reason>` (required) says how. Repeatable, each function once. A listed function whose shapes agree with the oracle's is a **FAIL** (a stale divergence). WP-509. |
| `ast-filter: <substr>` | `ast` only. Dump only the declarations whose qualified name contains `<substr>` (no whitespace): kiln gets `-fdump-ast-filter=<substr>`. |
| `pp-compare: tokens \| text` | `pp` only. How outputs are compared (see "pp comparison"). Default `tokens`. |
| `ast-implicit` | `ast` only. Include implicit declarations (implicit special members, injected-class-names, ...): kiln gets `-fdump-ast=implicit`, clang's dump is normalized as `xtask clang-ast --implicit`. Use with `ast-filter` to keep the builtin implicit declarations out. |
| `pp-linemarkers` | `pp` only. Preprocess with `-E` instead of `-E -P`, so the output keeps line markers, and compare them (see "pp comparison"). |
| `oven: [<line>[:<col>]] <counter><=N\|<counter>>=N ...` | kiln-specific constexpr budget (`<=` an upper bound, `>=` a floor); `<counter>` is `steps`, `bytes`, `peak_bytes`, `calls`, `allocs`, `peak_arena`, `frozen_bytes`, `static_bytes`, `evaluations`, `memo_hits` or `refl_bytes` (`memo_hits` and `refl_bytes` are budget-only counters the bench does not record; 0 in a report that lacks them). Not for `pp`. See "kiln-specific tests". |
| `inst-report: <golden.json>` | kiln-specific: the compile's template instantiation report must equal the golden, timings masked. Only `compile-ok`/`compile-fail`. See "kiln-specific tests". |
| `written-refs: <golden>` | kiln-specific: the compile's written-reference dump (`-fkiln-dump-written-refs`, spec/ast.md §18.6) must equal the golden. Only `compile-ok`/`compile-fail`. See "kiln-specific tests". |
| `decl-extents: <golden> [unordered]` | kiln-specific: the compile's declaration-extent dump (`-fkiln-dump-decl-extents`, spec/ast.md §19.7) must equal the golden; with `unordered`, as sorted lines (an importer built from a module image numbers its declarations in the image's order, WP-2091). Only `compile-ok`/`compile-fail`, and not with `written-refs:` (both dumps are the standard output). See "kiln-specific tests". |
| `pp-record: <golden>` | kiln-specific: the test's `-E -fkiln-dump-pp-record` output (spec/pp.md "Preprocessing record") must equal the golden. Only `compile-ok`, `compile-fail` and `pp`. See "kiln-specific tests". |
| `mem<=N` | Address-space budget of every process the test runs, in MiB (a positive integer; written without a `:` value). It lowers `--mem-limit` for this test (the cap is the smaller of the two) and applies even under `--mem-limit 0`. See "Resource limits". |
| `requires: target-<arch>` | Run only when the test's **effective target** has architecture `<arch>` (`aarch64` or `x86_64`; any other value is an ERROR). The effective target is the last `--target=<triple>`, `--target <triple>` or `-target <triple>` in the test's `args`; without one it is the compiler's default target, taken to be the host the harness runs on (`std::env::consts::ARCH`), as it is for kiln and clang. The triple's architecture is its first `-` component, with `arm64` read as `aarch64` and `amd64` as `x86_64`. Otherwise **SKIP** with the reason `requires target-<arch>; the test targets <arch> (`<triple>`)` or, without a `--target`, `requires target-<arch>; the default target is <host>`. For a `run` test whose expectations (diagnostic text, output) hold only on one target, since a run test cannot cross-run; a compile-only test pins `args: --target=<triple>` instead (WP-228), which then needs no `requires`. A `run` test that pins a `--target` other than the host's skips rather than failing to run its binary (WP-239). |
| `requires: host-<feature>` | Run only when the CPU the harness runs on has `<feature>` (`sve`; any other value is an ERROR) and the test's effective target (as for `target-`) has the host's architecture. The features are read from the `Features` (or `flags`) line of Linux's `/proc/cpuinfo`, or from the space-separated `KILN_HOST_FEATURES` when set; elsewhere the host has none. Otherwise **SKIP** with the reason `requires host-<feature>; the host CPU lacks <feature>` or `requires host-<feature>; the test targets <arch>, not the host`. Checked before `target-`. For a `run` test whose binary executes instructions of an optional extension (SVE, WP-810); such a test also carries `requires: target-aarch64`. |
| `requires: sanitizer-runtime` | Run only when every clang that links the test's program finds the UBSan and ASan runtimes (compiler-rt's `libclang_rt.ubsan_standalone` and `libclang_rt.asan` for the host, in either the `lib/linux/<name>-<arch>.a` or the per-target `lib/<triple>/<name>.a` layout; Debian ships them apart, as `libclang-rt-N-dev`). Those clangs are kiln's linker driver, `<llvm_dir>/bin/clang++` ("Oracle toolchain"), when kiln compiles the test, the compiler under test when it is clang, and the oracle when it is clang and the test compares with it; each is probed once per run. Otherwise **SKIP** with the reason `requires sanitizer-runtime; <role> (<path>) has no UBSan/ASan runtime`. The test's program runs with no address-space cap (`mem<=` and `--mem-limit` still bound the compiles): ASan reserves its shadow memory, gigabytes of address space, before `main`. Under **`KILN_REQUIRE_SANITIZER_RUNTIME=1`** (exactly `1`, as `KILN_REQUIRE_DEBUGGERS`; GitHub CI sets it) the missing runtime is an **ERROR** instead, `KILN_REQUIRE_SANITIZER_RUNTIME=1 and the test requires sanitizer-runtime; ...` (WP-1034). For `run` tests that link a sanitizer runtime (WP-1032). |
| `requires: clang>=X[.Y[.Z]]` | Run only when every clang the test runs is at least version `X.Y.Z` (missing components are 0). The clangs the test runs are the compiler under test when its kind is clang, and the oracle when its kind is clang and the test compares with it (`oracle` in effect, or `layout`/`vtables`/`ir`, and an oracle configured). The version is the `X.Y.Z` after `clang version ` in the `--version` line; a clang whose version cannot be read never meets the requirement. Otherwise **SKIP** with the reason `requires clang>=X.Y.Z; the compiler is clang <version>` (or `the oracle is ...`; `... names no clang version` when it cannot be read). With no clang involved (kiln without an oracle, or a non-clang oracle) the requirement is met. For tests that a known defect of an older clang breaks (docs/VALIDATION.md, known oracle defects; WP-239). |
| `requires: clang<X[.Y[.Z]]` | The complement: run only when every clang the test runs is OLDER than `X.Y.Z`; same clangs, same version reading (an unreadable version never meets it), and the SKIP reason `requires clang<X.Y.Z; the oracle is clang <version>`. Both bounds may appear (the lower is reported first). For an expectation a later clang release changed and kiln has not followed yet — the oracle-switch residual (docs/status/wp-184-oracle-switch.md); kiln alone (no clang run) still runs the test. WP-355. |
| `min-oracle: N` | Run only when every clang the test runs (the same clangs as `requires: clang>=`) is release `N` or later (`N` a positive major version); otherwise **SKIP** with the reason `min-oracle N; the compiler is clang <version>`. The line `crates/kiln_codegen/tests/ir_vs_clang.rs` reads for an IR case that follows a newer clang where an older one differs (that test compares such a case only under oracle `N` or later); kiln alone (no clang run) still runs the test, at its usual `-fclang-compat`. WP-405. |
| `module: <name>=<path>` | A module interface (or partition) unit the test imports, `<path>` relative to the test's directory (an absolute path is an ERROR; placeholders apply, so `std=%{libcxx_modules}/std.cppm` names libc++'s module source, WP-984); repeatable, each `<name>` once (a partition as `M:P`), listed in dependency order. Only for `compile-ok` (not `verify`), `compile-fail`, `no-crash` and `run` tests. Single-step (the default for compile kinds): kiln gets `-fmodule-file=<name>=<source>` (it builds the unit in-process, spec/modules.md part A); clang first precompiles each unit, in order, with `<clang> -std=<std> <args> <earlier units' -fmodule-file=...> --precompile -x c++-module <source> -o <scratch>/oracle-<name>.pcm` and then gets `-fmodule-file=<name>=<bmi>`. Two-step (`two-step`, and always for `run`; WP-917): kiln, too, writes module files, with `-fmodule-output=<scratch>/<name>.pcm -c -x c++-module <source> -o <scratch>/<name>.o` (`--precompile` alone when no object is needed), and gets `-fmodule-file=<name>=<scratch>/<name>.pcm`; clang compiles each `.pcm` with `-c` for its object. `:` in a name is `-` in the file name. Either way the flags follow `args` and precede the test file. A missing unit or one clang rejects is **ERROR**, one kiln rejects or crashes on **FAIL**; a compiler that is neither kiln nor clang **SKIP**s, and the GCC oracle is not run. The same lines `ir_vs_clang` reads (spec/codegen.md §6, "IR-diff corpus"). WP-405, WP-917. |
| `unit: <path>` | A further translation unit of a `run` test (an implementation unit, or a plain unit that imports the modules), `<path>` relative to the test's directory; repeatable. Each is compiled with `-c` after the test file, with the `module:` flags, to `<scratch>/[oracle-]unit<N>.o`; the program links the modules' objects, the units' and the test's (`-Qunused-arguments <args> <objects> -o ...`; for GCC as the compiler under test, `<args> <objects> -o ... -lstdc++exp`, its args in GCC's spelling and no `-Qunused-arguments`, which GCC rejects, WP-1261). The diags compared are the test file's compile's; a failing module, unit or link step fails the test (kiln) or makes it ERROR (oracle). A test with `unit:` lines and no `module:` or `header-unit:` line also runs under GCC as the compiler under test (WP-1261); any other compiler that is neither kiln nor clang SKIPs. WP-917. |
| `clang-unit: <path>` | A `unit:` file compiled by a clang rather than the compiler under test, for a program built from objects of two compilers (WP-1523). Same rules and order as `unit:` (the lines of both kinds are compiled and linked in the order written, the test file's object last), but the compile is `<clang> -std=<std> <args> -c <path>`: no `-fclang-compat`, no module flags. The clang is the oracle when it is clang (what the oracle lanes run), else the compiler under test when it is clang; without one the test **SKIP**s, as it does under GCC as the compiler under test. The oracle's own build of the test compiles the file with that same clang. Not with `module:` or `header-unit:` lines (an ERROR). |
| `link: fail` | The program of a `run` test with `unit:` / `clang-unit:` lines must not link (WP-1523): every compile succeeds and its diags match, then the link fails, and nothing runs. A link that succeeds FAILs the test (makes it ERROR for the oracle, which must fail to link as well). Needs a `unit:` or `clang-unit:` line, and no `stdout:`, `stdout-file:` or `exit:` (an ERROR otherwise). |
| `header-unit: "<path>"` / `header-unit: <name>` | A header unit the test imports (`import "<path>";` / `import <name>;`), for the same kinds as `module:`; repeatable, each spelling once; a quoted path is relative to the test's directory (absolute is an ERROR). Single-step, kiln gets nothing: it builds a header unit in-process at its import (spec/modules.md §3). Two-step (`two-step`, a `run` test, and `--bmi-kind`; WP-1802), kiln builds each the way a clang does (below, with its `-fclang-compat=`, `-fkiln-bmi-kind=N` and, under `--bmi-image`, `-fkiln-bmi-image`), into a header-unit module file (spec/modules.md §11.6), and gets the same `-fmodule-file=` flags. A clang (the compiler under test or the oracle) first builds each, in order, with `<clang> -std=<std> <args> -fmodule-header=user -x c++-user-header <test dir>/<path> -o <scratch>/[oracle-]hu<N>.pcm` (an angled name: `-fmodule-header=system -x c++-system-header <name>`, found on the include path), before any `module:` unit, and every later compile of the test (module units, the test file, `unit:` files) gets `-fmodule-file=<that file>` ahead of the `module:` flags. A `run` test with a `header-unit:` line is two-step (its test file is compiled with `-c`, then linked). A header unit the compiler under test rejects fails the test, one the oracle rejects or a missing file makes it ERROR; a compiler that is neither kiln nor clang SKIPs, and the GCC oracle is not run. clang warns `-Wexperimental-header-units` at the import, so tests pass `-Wno-experimental-header-units` in `args:` (kiln accepts and ignores it). WP-986. |
| `two-step` | No value. Compile kinds with `module:` or `header-unit:` lines only (a `run` test is always two-step; anything else is an ERROR): kiln compiles the units and header units to module files and imports those instead of the sources, so the test exercises the module file reader (spec/modules.md §11). WP-917, WP-1802. |
| `as-header-unit` | No value. `compile-ok` and `compile-fail` (not `verify`) only, else an ERROR: the test file itself is built as a header unit, so the rules that apply only while one is built ([module.import]/6) are checked. Every compiler's `-fsyntax-only` becomes `-fmodule-header=user`, the test file is given as `-x c++-user-header <file>` (a `.h` test gets no `-x c++`), and the module file goes to `<scratch>/[oracle-]main-hu.pcm`; the diags are checked as for any compile. Needs kiln or clang (any other compiler SKIPs). WP-1802. |
| `pch: <path>` | A precompiled-header test (WP-1023): `compile-ok`, `compile-fail` and `run` only (anything else is an ERROR), at most once, `<path>` relative to the test's directory. Before anything else the compiler under test, and a clang (or kiln) oracle (not for a test that is not compared with it, `oracle: none` or `skip(clang)`, WP-1462), each build `<program> -std=<std> [-fclang-compat=N] <args> -x c++-header <path> -o <scratch>/[oracle-]test.pch` (`-x c-header` for C); the test file's compile then adds `-include-pch <that file>` after the `module:` flags (`unit:` files do not get it). A producer the compiler under test rejects FAILs the test (so `xfail` applies); one the oracle rejects, or a missing header, is an ERROR. Needs kiln or clang as the compiler (else SKIP); no GCC comparison. The two steps are clang's own PCH workflow, so the oracle checks the same build shape kiln does. |
| `requires: llvm>=N` / `requires: llvm<N` | Run only when every LLVM backend that consumes the test's IR has major version at least `N` (`>=`) or below `N` (`<`); `N` is a bare major (`llvm>=22.1` is an ERROR), each form at most once. The backends are kiln's when kiln is the compiler under test or the compared oracle (the major of the oracle release whose `/usr/lib/llvm-N` kiln gets as `KILN_LLVM_DIR`, "Oracle toolchain") and every clang the test runs (as for `clang>=`; a clang's LLVM major is its own major, an unreadable version never meets it). Otherwise **SKIP** with the reason `requires llvm>=N; kiln's backend is LLVM <M>` (or `the compiler`/`the oracle`; `... has no known LLVM`). Checked after the `clang` requirements. For IR goldens whose syntax an older LLVM rejects (clang 22's `dead_on_return`, AArch64 `"frame-pointer"="non-leaf-no-reserve"`; spec/codegen.md §6a) and for a workaround only an older backend needs. Unlike `clang>=`, it is never met vacuously by kiln: kiln's backend always counts. WP-386. |
| `skip[(kiln\|clang\|gcc)]: <reason>` | Do not run. A qualified marker applies only to that compiler kind (`skip(gcc)`: when GCC is the compiler under test; it contradicts `oracle: gcc`, an ERROR). The oracle is a clang, so `skip(clang)` also implies `oracle: none`: a `run`, `pp` or `verify` test clang cannot compile is not compared with the oracle when kiln runs it (its own expectations, `stdout`/`exit`, `.expected` or `expected-*` comments, are the reference), and `oracle: clang` with `skip(clang)` is an error. `layout`, `vtables` and `ir` tests compare with the oracle only (no golden), so `skip(clang)` on them is an ERROR (`skip(clang)` does not apply to ... tests; a test neither compiler should run uses unqualified `skip:`). Running kiln alone would check nothing but that it compiles, which `compile-ok` already says. WP-258. |
| `xfail[(kiln\|clang\|gcc)]: <reason>` | Expected to FAIL. Qualified as for `skip`. |
| `meta-header: skip <reason>` | **SKIP** the test in the header-differential mode (`--meta-header`), with the reason `meta-header mode: <reason>`, and under GCC as the compiler under test (reason `GCC under test: <reason>`, WP-1249); no effect otherwise. For a test whose answer through GCC's library code is known to differ from kiln's built-in one (a step budget the real `std::vector` exceeds, wording that names a `kiln_meta.h` signature). A missing reason, or any other value, is an ERROR. WP-484. |

`expect`, `std`, `stdout-file`, `run-stdin-file`, `exit`, `oracle`,
`pp-compare`, `pp-linemarkers`, `ast-filter`, `ast-implicit`, `inst-report`, `written-refs`, `decl-extents`, `pp-record`, `mem<=`, `meta-header`, each `requires` kind (`target-`, `host-`, `clang>=`, `clang<`, `sanitizer-runtime`), `min-oracle`, `verify`, `two-step`, `as-header-unit`, each `module` name, `unit` path and `header-unit` spelling, and each `skip`/`xfail` qualifier may appear
at most once; `oven` at most once per
location (and once without one). If both a qualified marker
and an unqualified marker apply, the qualified one's reason is reported.

**Placeholders.** An `args:` flag or a `module:` path may contain
`%{name}`, replaced when the command is built:

| Placeholder | Value |
|---|---|
| `%{gcc_toolchain}` | The GCC oracle's install prefix (absolute; what `--gcc-toolchain=` expects): `<gcc> -print-search-dirs`'s `install:` directory four components up (`<prefix>/lib/gcc/<triple>/<version>/`), when that is a directory. Without one (no GCC configured, or a GCC whose prefix cannot be found) a test whose `args` name it is a **SKIP** (`no GCC oracle, so no %{gcc_toolchain} (the test needs GCC's libstdc++)`). For tests that compile against libstdc++ 16.1's real `<meta>` (`args: -freflection --gcc-toolchain=%{gcc_toolchain}` with `oracle: gcc`; WP-484); CI's GCC image gives `/usr/local`. |
| `%{libcxx_modules}` | The directory of libc++'s `std.cppm` and `std.compat.cppm` (absolute), from the `source-path` of the `libc++.modules.json` that `<compiler> -stdlib=libc++ -print-file-name=libc++.modules.json` prints. When that prints only the bare name, as clang++-22 without its own libc++ does, the fallback is the `*/c++/v1` directory of the `-stdlib=libc++ -E -v` search list, made canonical, then `<prefix>/lib/libc++.modules.json`. The tested compiler is asked first, then the oracle. Without one, a test that names it is a **SKIP** (`no libc++ module sources, so no %{libcxx_modules} ...`). For `import std;` tests that clang runs two-step (`module: std=%{libcxx_modules}/std.cppm`, WP-984); with libc++ 19 it is `/usr/lib/llvm-19/share/libc++/v1`. |
| `%{repo_root}` | The repo root (absolute). |
| `%{resource_dir}` | `<repo_root>/resource`, kiln's resource directory (the directory that contains `include/`, which is what `-resource-dir` expects). |
| `%{test_dir}` | The directory containing the test (absolute). |

An unknown name or a `%{` without a closing `}` is an ERROR at parse time.

**Caches.** Every compiler the suites run (`cargo xtask test`,
`external`, `libcxx`, and the commands of a libc++ `.sh.cpp` test) gets
`XDG_CACHE_HOME=<target>/xdg-cache`. So kiln's `std` module cache
(spec/driver.md "Module cache") is shared by the runs and never writes
to the user's `~/.cache` (`util::isolate_caches`, WP-984).
There is no escape for a literal `%{`. Values are absolute paths, so a
`pp-linemarkers` test whose golden must not depend on the checkout location
should name include directories repo-relative instead (cwd is the repo root).

The compiler kind is detected from the first line of `<compiler> --version`.
It is `kiln` if that line starts with `kiln version`, `clang` if it contains
`clang version`, `gcc` if it contains `(GCC)` or starts with `g++`/`gcc`,
and `other` otherwise.

### Diagnostics

The compiler's stderr is parsed as clang-format diagnostics:

- `file:line:col: sev: msg`, `file:line: sev: msg`, or location-less
  (`prog: error: msg` / `error: msg`).
- Indented lines (source snippets, carets, fix-its) are ignored.
- `fatal error` parses as severity `fatal`.

Diagnostics are grouped: a group is one primary (any non-note) followed by its
notes. The `diag:` directives are grouped the same way, and a `note` directive
must follow a primary directive.

Matching works as follows:

- Only diagnostics located in the test file count. Diagnostics in headers, in
  the predefines buffer, or without a location are ignored, except that a
  compile-fail needs one error in the test file (see below).
- A directive matches a diagnostic when the line is equal, the column is equal
  (if one was given), the severity matches, and the message contains the
  substring. The severity `error` also accepts `fatal`.
- A directive group whose primary carries `dup(clang)` counts as two
  identical directive groups when the compiler under test is `clang`, and
  as one otherwise.
- Each directive group is matched to a distinct diagnostic group (maximum
  bipartite matching). **Groups are order-insensitive.** clang can emit a
  diagnostic for a later line first; for example, the preprocessor runs ahead
  of the parser, so a `#warning` on line 9 is reported before a
  `static_assert` failure on line 5.
- Within a group, the note directives must match an in-order subsequence of
  the group's notes. Under `strict-diags`, they must match the notes exactly.
- Under `strict-diags`, every diagnostic group whose primary is located in the
  test file must be matched.
- With GCC as the compiler under test, no `diag:` directive is matched and
  `strict-diags` is not applied (see "GCC as the compiler under test").

## Test kinds

The compiler runs with cwd = repo root and receives the test as its
repo-relative name:

```
<compiler> [-x c | -x c++ (for .h)] -std=<std> <mode flags> <args...> <name> [-o <out>]
```

`.c` files get `-x c`, and `.h` test files get `-x c++`. A kiln-specific test
compiled by kiln also gets `-fconstexpr-report=<scratch>/constexpr-report.jsonl`
last (see "kiln-specific tests").

### C tests

A `.c` test (WP-1275; corpus `tests/c/`, a default root) is C: `std:`
takes `c89 c99 c11 c17 c23` (default `c17`); a GNU dialect is
`args: -std=gnuNN`, which wins as the last `-std=`. Every compile of it
runs the C driver beside the C++ one (`c_driver`: `clang++-19` →
`clang-19`, `g++-16` → `gcc-16`, in the same directory or on `PATH` for
a bare name; the program itself when there is none), so the oracle is
`clang-22 -x c` under kiln, a clang++ under test runs as clang, and GCC
links no libstdc++. kiln itself is run as given: `-x c` selects C. A
`unit:` file in the other language (a `.c` unit of a C++ test, a C++
unit of a `.c` test) is compiled as that language (`-x c` / `-x c++`,
which kiln++ needs to compile a `.c` as C) at its default standard
(`c17` / `c++26`) and linked with the test (`tests/c/interop/`). The
`expect: pp`, `ir` and `run` kinds and `skip(gcc):` work as for C++;
an `ir` test that matches only clang 22's attributes carries
`min-oracle: 22`.

Any compile ends as a **FAIL** ("compiler crashed" / "compiler hung") if:

- the compile is killed by a signal or times out, or
- stderr contains `internal compiler error: ` (kiln's crash banner,
  spec/driver.md "Internal compiler errors"), `panicked at` or `PLEASE
  submit a bug report`.

This holds whatever the expectation is, except under `expect: ice`. An
internal compiler error never satisfies `compile-fail`. kiln and clang,
as the compiler under test or the oracle, get `-fno-crash-diagnostics`
right after `-std=` on every compile (the test file, `module:`, `unit:`,
`header-unit:` and `pch:` builds), so a crashing test writes no
reproducer into `$TMPDIR` (WP-1264 added the clangs). GCC writes none and
gets nothing.

| Kind | Mode | Pass condition |
|---|---|---|
| `compile-ok` | `-fsyntax-only` | exit 0, and the diags match |
| `compile-ok` + `verify` | `-fsyntax-only -Xclang -verify` | exit 0 (every `expected-*` comment matched, nothing unexpected); the oracle's own check passes (below). A compiler that is neither kiln nor clang is a SKIP. |
| `compile-fail` | `-fsyntax-only` | non-zero exit, at least one `error`/`fatal` located in the test file, and the diags match |
| `run` | compile and link to `<scratch>/test.out` | the compile succeeds and the diags match; the binary exits normally within `--timeout`; stdout equals the expectation exactly (if one is given); the exit code equals `exit:` (default 0); the oracle agrees (below) |
| `pp` | `-E -P` (`-E` with `pp-linemarkers`) | exit 0; the diags match; the output is equivalent to `<test>.expected` (if that file exists); the oracle agrees (below). Equivalence is defined in "pp comparison". |
| `ast` | kiln: `-fsyntax-only -fdump-ast[=implicit] [-fdump-ast-filter=<substr>]`; clang: `-fsyntax-only -Xclang -ast-dump=json`, normalized | exit 0; the diags match; the dump equals `<test>.expected` up to trailing whitespace on each line and trailing blank lines; the oracle agrees (below). See "ast tests". |
| `cfg` | kiln: `-fsyntax-only -fkiln-dump-cfg`; clang: `-fsyntax-only -w -Xclang -analyze -Xclang -analyzer-checker=debug.DumpCFG` | exit 0; the diags match; the dump (stderr) equals `<test>.expected` as for `ast`; the oracle's graphs have the same shapes. See "cfg tests". Kinds other than kiln and clang SKIP. |
| `layout` | kiln: `-fsyntax-only -fdump-record-layouts`; clang: `-fsyntax-only -Xclang -fdump-record-layouts-complete` | exit 0; the diags match; the dump has at least one record layout; the oracle's dump has the same blocks in the same order. See "Layout and vtable tests". |
| `vtables` | kiln: `-fsyntax-only -fdump-vtable-layouts`; clang: `-S -emit-llvm -o <scratch>/test.ll -Xclang -fdump-vtable-layouts` | exit 0; the diags match; the dump has at least one vtable block; the oracle's dump has the same set of blocks. See "Layout and vtable tests". |
| `ir` | `-S -emit-llvm -O0 -o <scratch>/test.ll` (the oracle: `<scratch>/oracle.ll`) | exit 0; the diags match; the module defines at least one function; it equals the oracle's after normalization. See "IR tests". |
| `debugger` | compile and link with `-O0 -g` to `<scratch>/test.out`, then run gdb and/or lldb over it | the compile succeeds and the diags match; each debugger's output contains its checks in order. See "Debugger tests". |
| `ice` | kiln only: `-fsyntax-only` (none when `args:` hold `-c` or `-S`; then `-o <scratch>/ice.out`), `-fcrash-diagnostics -fcrash-diagnostics-dir=<scratch>/crash` | the compile crashes with kiln's internal-compiler-error banner and exits 1 (70 under `-fno-crash-diagnostics`); the `ice:` texts appear in order; the reproducer files the banner lists are exactly the files in `<scratch>/crash` (none under `-fno-crash-diagnostics`). See "ice tests". |
| `no-crash` | `-fsyntax-only -ferror-limit=0` (before the test's `args:`, so an explicit `-ferror-limit=N` there wins; a crash must not hide behind the error limit, WP-145) | exit 0 or 1 (errors anywhere are fine, in the test file or not), and the diags match; any other exit code (an internal-error exit such as 70 or 101) is a FAIL, as is a crash or hang (above). For robustness tests over input the compiler cannot fully handle yet, e.g. whole headers with `-ferror-limit=0` (WP-141). |

The `run` binary runs with cwd = its scratch dir, the `run-args:` arguments,
the inherited environment plus `env:`, and the `run-stdin:` /
`run-stdin-file:` bytes on stdin (empty without them). The oracle's binary
gets the same.

### Oracle toolchain

One environment variable, `KILN_ORACLE`, names the clang/LLVM release the
harness compares kiln with. It accepts `22` (the default when unset or
empty, WP-355) or `19` (the release the M3 gate is frozen on); any other value is an
infrastructure error (exit 2) naming the two accepted values, raised before
anything is built or run, by `xtask test`, `ci`, `stress`, `clang-ast`,
`gen-pp-corpus` and `libcxx` alike. The one place that maps the release to
tools is `xtask/src/oracle.rs`:

| Release N names | Used by |
|---|---|
| `clang++-N` | the default `--oracle` of `test`/`stress`; the `ci` self-check compiler (step 3); the default `--compiler` of `clang-ast` |
| `opt-N`, `llc-N`, `FileCheck-N` | named for the IR-diff and libc++ steps (WP-184 step E); `crates/kiln_codegen/tests/ir_vs_clang.rs` reads `KILN_ORACLE` itself (a test crate cannot use xtask) and runs `clang++-N`/`opt-N` |
| `/usr/lib/llvm-N` | exported as `KILN_LLVM_DIR` to every kiln++ the harness runs (`test` compiles and links with kiln as the compiler or the oracle; `libcxx` compiles, links and the `_LIBCPP_VERSION` probe), so kiln's headers, `llc`, `opt` and link driver are release N's (`kiln_driver/src/toolchain.rs`) |
| `/usr/lib/llvm-N/include/c++/v1` | the default `gen-pp-corpus --libcxx`; `libcxx` refuses to run (exit 2) when it is missing |

What a run passes, by role: the compiler under test gets the test's flags
and, when it is kiln, `KILN_LLVM_DIR=/usr/lib/llvm-N`; the oracle is
`clang++-N` with the same flags (an explicit `--oracle` replaces the
compiler, not N: kiln still gets release N's directory). Clang ignores
`KILN_LLVM_DIR`, so a clang under test or a clang oracle runs as before.
kiln_driver's own default (no `KILN_LLVM_DIR`) is the discovered
installation, LLVM 22 when installed (spec/driver.md "LLVM discovery",
WP-1016), so the harness's export only matters for oracle 19, and a
crate test that compares kiln with clang 19's IR or objects names
`--llvm-dir=/usr/lib/llvm-19` itself (`ir_vs_clang`'s `KILN_LLVM_DIR`).
`requires: clang>=N` compares with the oracle's `--version`, so it follows
the release without further configuration.

**The oracle follows a clang under test (WP-1512).** Without `--oracle`,
`xtask test --compiler clang++-M` (any clang whose `--version` major M is
a supported release, 19 or 22) is compared with `clang++-M`, and release
M's `/usr/lib/llvm-M` is the toolchain, whatever `KILN_ORACLE` says
(`Oracle::following`, called in `test::execute` after the compiler is
identified). That is how every hosted lane already runs: the `ci`
self-check is `--compiler clang++-N` against oracle N, and the clang 19
lane is not a corpus step at all but the `xtask` unit test
`seed_corpus_passes_with_clang` (`xtask/src/test/tests.rs`: compiler
`clang++-19`, oracle `clang++-19`, toolchain 19, over `tests/c`,
`tests/conformance`, the hand-written `tests/differential` and
`tests/stress`), run by the `unit xtask` job. Until WP-1512 a local
`--compiler clang++-19` was compared with the `KILN_ORACLE` default,
clang 22: every IR test (whose expectation is the oracle's module) and
every `requires: clang<22` twin then failed or skipped where the hosted
lane passes. With the rule, `requires: clang>=X` / `clang<X` are met or
not by the one release in play, so a version-bound test and its `-19`
twin are each checked by the lane of their release. An explicit
`--oracle` (or a caller's fixed toolchain, as the unit tests set) is kept
as given; kiln and GCC under test keep the `KILN_ORACLE` release.

**Two-oracle policy (WP-355).** The goldens, `//@ diag:` directives and
the IR-diff baseline are clang 22's, and the corpus is green under BOTH
releases: `cargo xtask ci` (oracle 22) and `KILN_ORACLE=19 cargo xtask ci
--quick --oracle-only` ("CI") are both gates. An expectation that differs between the releases
is pinned to one of them by `requires:` — `clang>=22` where clang 22's
output is the contract (a C++26 feature clang 19 lacks, a reworded
diagnostic, a re-sugared dump), `clang<22` where kiln has not followed
clang 22 yet (the residual list in docs/status/wp-184-oracle-switch.md).
A test that compares with the oracle (`oracle: clang`, `layout`,
`vtables`, `ir`) SKIPs under the other release; a kiln-only test (`diag`, a
golden without `oracle: clang`) still runs, with kiln at the level the
`requires:` names (below), so kiln is checked against the pinned
expectation under either oracle. An ERROR from the AST normalizer ("is not in
the mapping table") is a harness bug: `xtask/src/clang_ast/table.rs` lists the clang 22 JSON
attributes it ignores (`TemplateInstantiationPattern` on
`ClassTemplateSpecializationDecl`, `hasExplicitParameters` on
`LambdaExpr`; WP-262).

### Oracle

For `run`, the oracle compiles the test with the same flags into
`oracle.out`, which is then run. Its stdout must be byte-identical to the test
binary's, and so must its exit status. If the oracle's binary is killed by a
signal or exceeds `--timeout`, the test is an **ERROR**: the reference itself
is broken, so there is nothing to compare with.

For `pp`, the oracle preprocesses with the same flags, and its output must be
equivalent to the compiler's (see "pp comparison").

For `ast` (only with `oracle: clang`), the oracle's normalized dump must
equal the golden. It is checked when the compiler is kiln, or when the
oracle is not the compiler under test (a clang compiler's own dump is
already compared with the golden).

For `layout` and `vtables`, the oracle's dump is the reference; see "Layout
and vtable tests". For `ir`, the oracle's module; see "IR tests".

When the compiler is kiln and the oracle is a clang whose version is known,
every kiln command gets `-fclang-compat=<major>` ahead of the test's `args`:
the oracle's major version, unless the test's `requires: clang>=X` /
`clang<X` excludes it, in which case the nearest supported level that meets
the requirement (`requires: clang>=22` under oracle 19 runs kiln at 22,
`clang<22` under oracle 22 runs it at 19; `runner::compat_for`), so kiln's predefined macros and `__has_*` answers
are the oracle's (spec/pp.md "clang compatibility level"; WP-263). A test's
own `args: -fclang-compat=N` comes later and wins. Without an oracle
(`--no-oracle`, or a kiln oracle) kiln runs at its default level.

**Oracle crash retry.** A clang oracle can crash nondeterministically (an
address-dependent clang 19 bug, docs/VALIDATION.md "known oracle defects";
WP-1254), so a compile by a clang oracle that dies of a signal is run once
more with the same command. That covers every compile the oracle runs:
`run`'s `oracle.out`, its verdict on a `compile-ok` or `compile-fail` test,
its `pp`, `ast`, `layout`, `vtables` or `ir` output, and its module,
header-unit, PCH or two-step builds. "Dies of a signal" means either of these:

- The process is killed by a signal.
- The driver reports that one of its commands was: `... command failed due
  to signal`, with the signal named by `unable to execute command: <signal
  description>` (Debian's clang runs `cc1` out of process and then exits 1),
  or `... failed with exit code 128+N`.

If the retry succeeds, the test goes on as if the first compile had, and
its result line carries `[advisory: oracle crashed once (signal N), retry
ok]`. Like a GCC advisory ("GCC oracle"), this is printed even when the
test passed, and the summary counts it ("Output"). If the retry crashes
too, the test stays an **ERROR**, as any oracle crash is.

There is never more than one retry, and the success path runs one process.
None of these are retried:

- a timeout
- a memory kill: running out of memory under the cap ("Resource limits"),
  `LLVM ERROR: out of memory`, `std::bad_alloc`, or SIGKILL (the kernel's OOM
  killer)
- a failure that is not a signal: a rejection, or another exit code
- the run of the oracle's binary
- a kiln oracle
- the compiler under test, kiln or not, even when it is the same clang as
  the oracle

Every compile by a clang oracle (or a kiln one) gets `-fno-crash-diagnostics`
right after `-std=`, as the compiler under test does ("Crashes"), so a
crashing oracle leaves no preprocessed-source and script reproducers in
`$TMPDIR` of a CI runner (WP-1264); a test's own `args:` come later and win.

The command log shows `# the oracle crashed (signal N); retrying it once`
and, when the retry crashes too, `# the oracle crashed again (...); not
retried` (`xtask/src/test/oracle_retry.rs`; self-test
`xtask/fixtures/oracle-crash/`).

### GCC oracle

GCC 16.1 is the reference implementation of C++26 reflection (P2996,
P3394, P3491, P3293; docs/plans/m6-reflection.md D1), which no clang
release implements. A test that says `oracle: gcc` is compared with it
instead of clang, when kiln is the compiler under test and a GCC is
configured (`--gcc`, `KILN_GCC`, or `g++-16` on `PATH`; `scripts/env.sh`
exports `KILN_GCC=.toolchain/gcc-16.1/bin/g++-16` on the dev host when
it is installed there). GCC compiles the test **once**:

| Kind | GCC runs | Compared |
|---|---|---|
| `compile-ok` / `compile-fail` | `-fsyntax-only` | the accept/reject verdict: GCC rejecting a `compile-ok` test, or accepting a `compile-fail` one, is **ERROR** (the expectation is in doubt) |
| `run` | a compile and link, then one run of the binary with the test's `run-args`, stdin and `env` | the binary's stdout and exit status, which must equal kiln's binary's (a difference is a FAIL; GCC rejecting the test is ERROR) |

IR and diagnostic wording are **never** compared with GCC (`diag:` and
`strict-diags` still check kiln's own diagnostics). The command is
`<gcc> [-x c|c++] -std=<std> <mode> <args> <file> [-o <out>]`, where
`<args>` are the test's with the kiln- and clang-only flags dropped
(`-fclang-compat=`, `-fkiln-*`, `-fconstexpr-report=`, `-stdlib=`,
`-resource-dir <dir>`, `-Xclang <arg>`, `--gcc-toolchain=<dir>`: GCC
uses its own library), two renamed
(`-ferror-limit=N` to `-fmax-errors=N`, `-fconstexpr-steps=N` to
`-fconstexpr-ops-limit=N`), and clang's diagnostic-only flags, which GCC
rejects, dropped too (WP-1261): `-Qunused-arguments`, `-Weverything`,
`-f[no-]crash-diagnostics[=...]`, the `-R<remark>` flags, and every
`-W<name>`, `-Wno-<name>`, `-Werror=<name>` and `-Wno-error=<name>` whose
`<name>` (up to any `=`) this GCC does not list for the test's language
(`<gcc> -Q --help=warnings -fsyntax-only -x c|c++ /dev/null`, read once per
run; a GCC that lists nothing keeps them all). `-Wno-everything` becomes
`-w`. The map is `oracle::gcc_args_with`. Reflection is `-std=c++26 -freflection` in
both compilers (verified on GCC 16.1.0); libstdc++'s `<meta>` is GCC's
default library.

**Oracle advisory.** A GCC crash (a signal), `internal compiler error`,
`sorry, unimplemented`, timeout or memory kill never fails kiln (the
WP-102 policy for a pre-release oracle): the comparison is dropped, the
test's result is whatever kiln's own expectations give, and the result
line carries `[advisory: oracle crashed (<why>)]`; such a result is
printed even when it passed, and the summary counts it ("Output"). GCC is
never retried: its crash drops the comparison, where a clang oracle's
crash is retried once and then is an ERROR ("Oracle crash retry").

Without a GCC (none configured, `--no-gcc`, `--no-oracle`) an
`oracle: gcc` test runs against its own expectations only; it never
fails for want of GCC, unless **`KILN_REQUIRE_GCC=1`** is set (exactly
`1`, as `KILN_REQUIRE_LLVM`): then `cargo xtask test` and `cargo xtask ci`
exit 2 before running anything, with `KILN_REQUIRE_GCC=1 and no usable
GCC oracle ...` naming the fix. "Usable" is the WP-483 check: the
configured GCC runs and `--version` says GCC; a `--gcc`/`KILN_GCC` that
does not is an error with or without the variable, and `--no-gcc` /
`--no-oracle` under it are the same error. `xtask test` reads the
variable in `main` (`Options::require_gcc`), so the in-process API and
the harness's own tests are unaffected by the caller's environment. GitHub
CI sets `KILN_GCC=/usr/local/bin/g++` (the `gcc:16.1.0-trixie` image) and
`KILN_REQUIRE_GCC=1` in both jobs ("CI", WP-502); `ci` and `test` resolve
GCC the same way, so a local `cargo xtask ci` with `scripts/env.sh`
sourced runs the same comparisons. The header line names the GCC and its
version, and the summary counts the comparisons ("Output"), so a log
shows whether the oracle ran. `tests/conformance/meta.reflection.ci/`
holds the `oracle: gcc` tests that need nothing past the reflection on
`main` when CI first ran GCC (WP-502).

### GCC as the compiler under test

`cargo xtask test --compiler $KILN_GCC <paths>` runs the corpus with GCC
(kind `gcc`, detected from `--version` as for the GCC oracle) instead of
kiln: the implementer's check that a reflection or contracts test's
expectation is GCC 16.1's too. It is not a CI step. It mirrors
`--compiler clang++-N`, minus what is kiln's and clang's own:

- **Flags.** Every command of the compiler under test (the test file's
  compile, a `unit:` file's, the two-step link) gets the mode's flags and
  the test's `args` in GCC's spelling, mapped exactly as for the GCC
  oracle ("GCC oracle": `oracle::gcc_args_with`; `--gcc-toolchain=`, the
  other kiln/clang-only flags and the clang diagnostic-only flags dropped,
  `-ferror-limit=`/`-fconstexpr-steps=` renamed), so `no-crash`'s
  `-ferror-limit=0` reaches GCC as `-fmax-errors=0`; no flag reaches GCC
  raw (WP-1261). A compile that links (no `-c`, `-S`, `-E` or
  `-fsyntax-only`), and the two-step link, also get `-lstdc++exp` last,
  since GCC keeps the default contract-violation handler and
  `invoke_default_contract_violation_handler` there; the link gets no
  `-Qunused-arguments` (GCC rejects it and has no unused-argument warning
  to silence). GCC gets no `-fno-crash-diagnostics` either: it writes no
  reproducer. The clang oracle, when the test compares with one, gets the
  test's flags unchanged.
- **The flag probe.** A test with `args` first has GCC compile a one-line
  file, `<gcc> -std=<std> -fsyntax-only -I<stand-in dir> <mapped args> -x
  c|c++ <scratch>/gcc-flag-probe.{c,cpp}`. If GCC still rejects a mapped
  flag that has no GCC equivalent and that dropping would change the
  test's meaning (`-fsanitize=cfi`, `-flto=thin`, `--target=`,
  `-fsanitize-ignorelist=`, `-fprofile-instr-generate`, ...), its error is
  not located in a file (`g++: error: unrecognized command-line option
  ...`, `cc1plus: error: ...`), and the test is **SKIP**ped with the reason
  `GCC under test: GCC rejects the test's flags (<that line>)`. A probe
  that fails otherwise, or cannot run, lets the test run as usual. The
  probe runs after every other skip and is logged with the test's
  commands.
- **The stand-in `kiln_meta.h`.** `-I<repo_root>/tests/support/meta-header`
  comes first on GCC's command line, before the test's `args`, as in the
  header-differential mode, so a reflection test's `#include "kiln_meta.h"`
  reads the stand-in over GCC's own `<meta>` instead of kiln's header (whose
  intrinsics and `__kiln_*` extensions GCC does not have). A test that
  includes a `kiln_meta*.h` is **SKIP**ped, with a reason starting `GCC
  under test: `, for exactly the reasons that mode skips it (it says
  `meta-header: skip <reason>`; it is not `compile-ok`, `compile-fail`,
  `run` or `no-crash`; it passes `-stdlib=libc++` or `-nostdinc*`; it names
  a kiln extension). A test that does not include one is unaffected.
- **What is checked, per kind.** `compile-ok`: exit 0. `compile-fail`:
  a non-zero exit and at least one `error`/`fatal` located in the test
  file (GCC prints `file:line:col: error: msg`, parsed as in
  "Diagnostics"). `no-crash`: exit 0 or 1. `run`: the
  compile succeeds, the binary's stdout and exit code meet `stdout:`/
  `stdout-file:`/`exit:`, and the clang oracle (`oracle: clang`, the
  default) agrees on both. `debugger`: the debugger scripts' output, as
  under clang. `pp`: the output and the oracle comparison, as
  under clang, after the line marker of `stdc-predef.h` (which GCC
  includes, `#define`s only, before every hosted translation unit; kiln
  and clang include nothing) is dropped from GCC's `-E` output
  (`runner::without_gcc_preinclude`, WP-1261). A crash or hang is a
  FAIL, as for any compiler.
- **Not checked: `diag:` and `strict-diags`.** Their text is kiln's and
  clang's wording (spec/diag.md), which GCC does not share, so under GCC
  the `diag:` list is not matched and `strict-diags` is not applied
  (`runner::diag_directives_for` gives `None`). No directive opts a test
  back in; none is needed, since accept/reject and run output are what a
  GCC run is for.
- **Not run: SKIP**, with the reasons above: kiln-specific tests (`oven:`,
  `inst-report:`, `written-refs:`, `decl-extents:`, `pp-record:`), `verify`, `ast`, `cfg`, `layout`, `vtables`, `ir`,
  `ice`, `module:`/`header-unit:` and `pch:` tests (a test with only
  `unit:` lines runs, built in two steps, WP-1261; "Test kinds",
  "kiln-specific tests"). So is a test that is itself a module unit or
  imports one, which kiln and clang take with `-std=c++20` and up but GCC
  only with `-fmodules` (its CMIs going to `gcm.cache/` in the working
  directory, which the harness does not drive): a line of the test file
  that, left-trimmed and without a leading `export `, opens `module;`,
  `module <name>`/`module :<partition>` or `import <name>`/`import
  :<partition>`/`import <header>`/`import "header"` ([module.unit]/1,
  [module.import]/1; `module::` and `import =` are identifiers), and a
  test whose `args` name `-fmodule-file=` or `-fprebuilt-module-path=`
  (clang module flags; GCC reads `-fmodule-file=` as a D option). The
  reason names the line or the flag (`runner::gcc_module_syntax`,
  WP-1261). `oracle: gcc` is not compared (GCC is already the
  compiler under test); such a test runs against its own expectations.
- **Markers.** `skip(gcc)` and `xfail(gcc)` apply (a GCC disagreement
  with kiln's expected behaviour is an `xfail(gcc): <reason>`), as do the
  unqualified ones; `xfail(kiln)` and `skip(kiln)` do not.

### Header-differential mode

`cargo xtask test --meta-header [paths...]` (WP-484) runs the reflection
tests against libstdc++ 16.1's real `<meta>` instead of kiln's own
declarations in `tests/support/kiln_meta.h`, so a `<meta>` function
kiln's header declares differently from GCC's (a signature, a library
function kiln implements as an intrinsic, a layout) shows up as a
failing test. The mode:

- puts `-I<repo_root>/tests/support/meta-header` and
  `--gcc-toolchain=<prefix>` (the GCC oracle's, as `%{gcc_toolchain}`)
  first on kiln's command line, before the test's `args`. That directory
  holds a stand-in `kiln_meta.h` (and `kiln_meta_queries.h`,
  `kiln_meta_traits.h`) that includes the real `<meta>` and the other
  headers kiln's declares, defines the test helpers of kiln's header
  that are not in the standard (`__kiln_info_array`,
  `__kiln_data_members`, `name_is`) over the standard API, and defines
  `KILN_REAL_META`. A test's `#include "kiln_meta.h"` finds the stand-in
  first, so no test text changes: line numbers and file names in
  diagnostics stay the same, and `diag:` expectations still apply.
- passes `-I<dir>` (not the toolchain) to the GCC oracle too, so GCC
  reads the same stand-in.
- does not compare with the clang oracle, and does not check `oven:`
  budgets (they are measured over kiln's built-in calls; the real
  header's library code costs more steps and allocations).
- **SKIP**s, with a reason starting `meta-header mode: `, a test that:
  does not include a `kiln_meta*.h`; says `meta-header: skip <reason>`;
  is not `compile-ok`, `compile-fail`, `run` or `no-crash` (an `ast`,
  `ir` or `pp` test compares the declarations of kiln's header
  themselves); passes `-stdlib=libc++` or `-nostdinc*`; or names a kiln
  extension: an identifier `__kiln_*` other than the stand-in's helpers,
  or a call of a non-`__kiln_` extension kiln's header declares
  (`name_of(`).

A test whose answer through the real header is known to differ guards
the difference with `#if defined(__KILN__) && !defined(KILN_REAL_META)`
or opts out with `meta-header: skip`. The mode is a local and release
check (docs/VALIDATION.md), not part of `cargo xtask ci`.

### Debug-info sweep

`cargo xtask test --debug-sweep [paths...]` (WP-910) checks that `-g`
never breaks a program kiln compiles. For each test it:

- **SKIP**s a test that is not `compile-ok`, `run`, `ir`, `layout`,
  `vtables` or `debugger` (reason ``not swept (`expect: <kind>`,
  `--debug-sweep`)``), after the usual `requires`/`skip` gates;
- compiles the test with its `args` and `-c` to an object, without
  `-g`. The arguments that stop a compile before the backend lowers
  LLVM's own intrinsics or that write something other than an object
  (`-Xclang -disable-llvm-passes`, `-emit-llvm`, `-S`: an `expect: ir`
  test's) are dropped and the rest kept, so an IR test is compiled to an
  object and checked like any other (WP-1563). A test that does not
  compile to an object (an `xfail(kiln)`, a `-fsyntax-only` in its
  `args`) is a **SKIP**, ``does not compile to an object without -g
  (<end>)``, unless that compile **crashed**: killed by a signal, timed
  out, exit status 70 (an internal compiler error under
  `-fno-crash-diagnostics`), or a crash report on stderr (`panicked at`,
  `internal compiler error: `, `PLEASE submit a bug report`, a
  reproducer's `note: diagnostic msg: ` lines). A crash is a **FAIL**
  with the compiler's stderr: a checked-mode panic is never a skip. The
  one exception is a crash in LLVM's backend (no panic, no internal
  compiler error, not status 70) that the control (the toolchain's clang,
  the same source and arguments, `-c`) crashes on too: a **SKIP** naming
  it, as `__builtin_coro_size` outside a coroutine is in both;
- compiles it again with `-c -g`, which must succeed and write the
  object, else **FAIL** with the compiler's stderr;
- runs `llvm-dwarfdump --verify` on the `-g` object (the toolchain's
  `bin/llvm-dwarfdump`, else `llvm-dwarfdump-N` on `PATH` for the
  toolchain's major version, else none, and the sweep only compiles); a
  verifier error is a **FAIL** with its output, unless the control (the
  toolchain's clang, the same source, arguments and `-c -g`) writes an
  object the verifier rejects too: that defect is LLVM's, and the test
  is a **SKIP** naming it. An object that is LLVM bitcode (`-flto`) has
  no DWARF sections to verify; the `-g` compile succeeding is the check.

`xfail` does not apply (a test is swept only when it compiles), and
neither do diagnostics, oracles, run output or `oven:` budgets. Both
compiles run under the test's memory cap and the compile timeout; a
crash or hang is a FAIL as in any mode. The sweep is a local and release
check (docs/VALIDATION.md), not part of `cargo xtask ci`: the whole
conformance corpus takes several minutes.

### verify tests

A `verify` test states its expectations once, as clang `expected-*`
comments, and both compilers check them with their own `-verify`. The
compiler's check failing is a **FAIL**, with its `-verify` report as the
detail. With `oracle: clang` (the default), the oracle then runs the same
command. If its check fails, the test is an **ERROR**: the comments do not
describe clang 19, so the test is wrong. As for `ast`, the oracle is
skipped when the compiler under test is the oracle itself. `--no-oracle`
and `oracle: none` skip it. `//@ diag` tests are unaffected. Use `verify`
where the expected diagnostics should be pinned for both compilers (notes,
regex matches, diagnostics in headers), and `diag:` where kiln's
diagnostics deliberately differ from clang's.

The oracle is run even when the compiler under test is itself clang. This
keeps the harness's own code paths exercised. If the oracle compile fails or
hangs (exceeds the compile timeout), the test is an **ERROR**: the test is
wrong, not the compiler.

### pp comparison

Two preprocessed outputs are compared in one of two modes, chosen by the
`--pp-compare` option, else the test's `pp-compare:` directive, else
`tokens`. The mismatch report names the mode.

**`tokens`** (the default; `xtask/src/test/pp_tokens.rs`). Both outputs are
re-lexed with `kiln_lex` under the test's standard (`c++NN` as written; C
standards use the C++98 rules until `kiln_lex` has a C mode) and compared as
sequences of preprocessing tokens:

- Whitespace, comments and line breaks are not compared, except as token
  boundaries. `a+b` and `a + b` are equal; `++` and `+ +` are not.
- Punctuators compare by kind, so a digraph equals its primary spelling
  (`<:` equals `[`, `%:%:` equals `##`).
- Every other token compares by category (identifier, pp-number, character
  literal, string literal, raw string literal, header name, other character,
  invalid) and spelling. The spelling is taken after removing line splices
  (and trigraphs, where the standard has them), and is otherwise exact: a
  UCN and the character it names are different spellings, as are `0x1F` and
  `0X1f`.
- Without `pp-linemarkers` (`-P`), nothing else is compared.
- With `pp-linemarkers`, lines of the form `# N "file" flags` (and
  `#line N "file"`) are parsed instead of compared as tokens. Every token
  gets a presumed location (file, line) and the system-header flags (3, 4) in
  effect, and those are compared too. A marker that enters (flag 1) or
  leaves (flag 2) a file is itself an item, compared by line, file and
  flags, except markers that enter or leave a pseudo-file (`<built-in>`,
  `<command line>`), which differ between compilers. Markers without flag
  1 or 2 only move the presumed location, so the choice between a marker and
  blank lines is not compared. File names are normalized by collapsing
  `//`, `/./` and `dir/..`.

On a mismatch the report gives the index of the first differing item and
the item counts, three items of context, then the differing items, `-` for
the expected side (the golden or the oracle) and `+` for the actual one, as
`line:col  spelling  (category)`; in marker mode each token also shows
` [file:line]` (plus ` sys` or ` sys extern-C`), and a marker item shows as
`# N file flags  (line marker)`. The
items stop where the two sides agree again (8 equal items within 64 of
each side), or after 10. The report ends with the physical output line of
the first difference on each side.

**`text`** (`compare::pp_diff`). Runs of spaces and tabs collapse to one
space, lines are trimmed, and blank lines are dropped; the results must be
equal, and the report is a line diff. Use it for a test whose point is the
exact layout of the output (the printer's spacing), not its tokens.

### Goldens

A pp or ast golden is `<test file name>.expected`, for example
`function-like.cpp.expected`, stored next to the test. `--update` rewrites an
existing golden with the raw actual output when the comparison above says it
differs (so a golden is not churned by spacing changes), and reports
"updated N .expected file(s)". It never creates a golden. To opt a test into a
golden, create an empty file and run `--update`, then review the diff.
(For ast tests the golden is required, so this is also how one is created;
`cargo xtask clang-ast <test> > <test>.expected` does the same.)

### ast tests

An `ast` test compares the AST dump (`spec/ast-dump.md`) with its golden
`<test>.expected`, which is required: a missing golden is an **ERROR** whose
message points to `cargo xtask clang-ast <test>`, which prints the golden as
clang sees it. Under kiln the dump is kiln's stdout. Under clang, clang's JSON
dump is converted in-process by the `xtask clang-ast` normalizer
(`spec/ast-dump.md` §6; constexpr values come from a second, text-dump run),
so the AST corpus is checked against clang today. A dump the normalizer
cannot convert (a construct it does not map) is an **ERROR** naming the
construct: the test cannot be checked against clang. With another compiler
(`other`), `ast` tests are a SKIP. A dump that differs from the golden is a
**FAIL** with a line diff (`-` golden, `+` actual); under `--update` the
golden is rewritten instead.

### cfg tests

A `cfg` test checks the control-flow graphs kiln builds for each function
body (spec/sema.md §7.5.1), printed by `-fkiln-dump-cfg` in clang's
`DumpCFG` layout. Its golden `<test>.expected` is kiln's exact dump
(required; `--update` writes it). The dump's element texts are kiln's own,
so clang's dump is compared by *shape* only:

- Each graph is split into blocks. A block's shape is its tag (`entry`,
  `exit`, `noreturn`), whether it starts with a label, its terminator's
  kind (`if`, `while`, `do`, `for`, `switch`, `try`, `break`, `continue`,
  `goto`, `goto*`, `?:`, `&&`, `||`, `tempdtor`, the first word otherwise)
  and its successors, each marked `(Unreachable)` (a pruned edge) or
  `NULL` (a dropped loop edge). Elements and predecessors are ignored.
- Blocks are renumbered canonically, by a depth-first walk from the entry
  over the successors in order, then from the blocks it missed in print
  order, so the two numberings need not agree.
- Graphs are matched by function name: the identifier before the
  parameter list, without template arguments or qualifiers, then by order
  among graphs of the same name. Lambda call operators are not compared
  (clang's analyzer dumps none); they are checked by the golden alone.

Under kiln with an oracle, the oracle's shapes must equal those of kiln's
dump, except for the functions listed in `cfg-diverges`; a listed function
whose shapes agree is reported as stale. Under clang, clang's shapes must
equal the golden's (a listed function is skipped, never stale). The known
divergences by construction: clang's analyzer adds a `static init`
branch for a local static and virtual-base branches in constructors
(`AddStaticInitBranches`, `AddVirtualBaseBranches`, which its warnings'
graph does not), and clang orphans the destructor decision of a
temporary nested inside a `?:` arm that has none of its own.

### Layout and vtable tests

`layout` and `vtables` tests check the Itanium C++ ABI dumps
(`spec/abi.md` §7) against the oracle's. There is no golden: clang's dump is
the reference, produced from the same source and flags (`std:`, `args:`) in
the modes of the table above. kiln's plain `-fdump-record-layouts` already
lays out every complete record, like clang's `-complete` variant; clang only
dumps the vtables it emits, so its side compiles to LLVM IR, and every
dynamic class in a `vtables` test needs a key function defined in the test.

Both dumps are split into blocks by `kiln_abi::dump_blocks`, the splitter the
`kiln_driver` differential tests use:

- **layout**: a block starts at each `*** Dumping AST Record Layout` marker.
  clang's builtin records (`__NSConstantString_tag`, `__va_list_tag`,
  `std::__va_list`) are dropped. The blocks must be equal, one by one and in
  order. A mismatch is a **FAIL** that names the first differing block (or
  the first one only one side has) with both texts and a line diff
  (`-oracle +actual`), plus the two block counts.
- **vtables**: a block starts at each `Vtable for `, `Construction vtable
  for `, `VTable indices for `, `Thunks for ` or `Virtual base offset
  offsets for ` line. clang's `Original map` sections are dropped, trailing
  blank lines are trimmed, and the blocks are compared as a set, because
  clang prints them in emission order and repeats construction vtables. A
  mismatch is a **FAIL** listing, for each pair of blocks with the same head
  line, a line diff, then the blocks only the oracle has and the blocks only
  the compiler has (at most 4 of each, then a count).

The compiler under test must dump at least one block (a test that pins
nothing is a **FAIL**), and so must the oracle: an empty oracle dump is an
**ERROR** (for `vtables`, usually a class without a defined key function). A
failing compile is a **FAIL** under the compiler and an **ERROR** under the
oracle. The comparison runs, as for `ast`, when the compiler is kiln or the
oracle is not the compiler under test; in the clang self-check the dump must
still be produced and non-empty. Without an oracle (`--no-oracle`) these
tests are a **SKIP**, and with another compiler (`other`) too. `oven:`
budgets apply as usual (kiln's `-fsyntax-only` compile is the one checked).

### IR tests

`ir` tests (WP-386) pin kiln's LLVM IR to the oracle's exactly: both
compile the test with `-S -emit-llvm -O0` (plus `std:`, `args:` and, for
kiln, the usual `-fclang-compat`/`KILN_LLVM_DIR`) for the default target,
named explicitly as `--target=<host arch>-unknown-linux-gnu` to both unless
`args:` gives a `--target` (the vendor of clang's default triple is a
build setting: Debian's x86-64 clang says `x86_64-pc-linux-gnu`; unit
test `ir_tests_name_the_target_triple`), and the two modules must be equal after dropping comment lines and
trailing comments (`; preds = ...`, `; No predecessors!`), the `; ModuleID` and
`source_filename` lines, `!llvm.ident` and the producer string node
(`!N = !{!"... clang version ..."}` / `kiln version`). Nothing else is
normalized: attributes, attribute group numbers, metadata, alignment,
flags, value names and order must match. A mismatch is a **FAIL** with a
line diff (`-oracle +actual`). They are the exact goldens for codegen that
only a newer clang emits (spec/codegen.md §6a): pin them with
`requires: clang>=22` and, where LLVM 19 cannot parse the IR,
`requires: llvm>=22`. The rest is as for `layout`: the compiler must
define at least one function, a failing oracle compile is an **ERROR**,
and the test is a **SKIP** without an oracle or under `other`.

### Debugger tests

`debugger` tests (WP-814) check that a `-g` binary debugs as clang's
does: the test is compiled and linked with `-O0 -g` (plus `std:`,
`args:`, the usual `-fclang-compat`), and each debugger with commands
runs over `<scratch>/test.out`, with cwd = the scratch dir, under
`--timeout` but never less than 60 s (`DEBUGGER_TIMEOUT`: a debugger
takes seconds to load the binary and, for lldb, its Python runtime, more
on a busy CI runner; a session that runs out is a **FAIL** naming the
timeout, with the output so far), and with `DEBUGINFOD_URLS` removed
from its environment (no symbol-server lookups, so nothing depends on
the network): `gdb -batch -nx -x <scratch>/gdb.cmd test.out` and
`lldb -b -s <scratch>/lldb.cmd test.out` (`lldb-19`, else `lldb`, on
`PATH`). The script is the `gdb:` (`lldb:`) lines, one command per line.
The output, stdout then stderr, must contain each `gdb-check:`
(`lldb-check:`) substring on a line after the line the previous check
matched; the first missing check is a **FAIL** that shows the output.
The debugger's exit code is not checked (a batch debugger exits 0 after
a failed command; the checks catch it). There is no oracle comparison:
the same test passes under `--compiler clang++-19`, which is the
reference. A test whose debugger is not on `PATH` is a **SKIP**
(`requires `gdb` on PATH (`expect: debugger`)`), so a machine without
gdb or lldb-19 still passes the corpus, with the reason shown. Under
**`KILN_REQUIRE_DEBUGGERS=1`** (exactly `1`, as `KILN_REQUIRE_GCC`) it is
an **ERROR** instead (`KILN_REQUIRE_DEBUGGERS=1 and `gdb` is not on PATH
(`expect: debugger`)`), so the run fails. GitHub CI sets it in every job
and installs gdb and lldb-19 in the jobs that run the corpus ("CI",
WP-816), so the debugger tests cannot silently skip there. Like
`KILN_REQUIRE_GCC`, only the entry points (`cargo xtask test`, the `ci`
corpus runs) read it, never the harness's own tests.
`tests/debuginfo` is a default root, so `cargo xtask test` and both `ci`
corpus runs include it (WP-816; before, it had to be named).

The debuggers need `ptrace` on the test binary they start. A container
allows that under Docker's default seccomp profile (kernel 4.8 and
later), and the development container runs the tests with neither
`CAP_SYS_PTRACE` nor `seccomp=unconfined`; GitHub CI adds
`--cap-add=SYS_PTRACE` to the jobs that run them anyway, so `ptrace` does
not depend on the host's Yama setting.

Write checks that hold on both targets and both compilers: source text
and line numbers rather than addresses, `this=0x` rather than a pointer
value. lldb in a container cannot disable ASLR (`personality` is
blocked), so lldb scripts start with `settings set target.disable-aslr
false`; prefer `frame variable` / `target variable` to `expression`,
which needs a JIT. gdb `-batch` does not print `finish`'s "Run till
exit" banner. The corpus is `tests/debuginfo/`.

### ice tests

`expect: ice` checks kiln's crash path (spec/driver.md "Internal compiler
errors"; WP-911): the test's `args:` name the hidden
`-fkiln-crash-for-testing=<phase>`, which makes kiln panic on purpose in
that phase. The harness removes `<scratch>/crash`, compiles with
`-fcrash-diagnostics -fcrash-diagnostics-dir=<scratch>/crash` (after the
default `-fno-crash-diagnostics`, before the test's `args:`, so a test's own
`-fno-crash-diagnostics` wins), and the test PASSes when:

- stderr contains `error: internal compiler error: ` (the crash checks
  of "Test kinds" do not apply);
- the exit code is 1, or 70 when the last `-f[no-]crash-diagnostics[=…]`
  of `args:` turns crash diagnostics off;
- each `ice:` text is found after the previous one's match;
- the absolute paths on the banner's `note: diagnostic msg: ` lines (its
  other `diagnostic msg:` lines are messages, as clang's) are exactly the
  files in `<scratch>/crash`: at least one normally, none when crash
  diagnostics are off.

Any other compiler is a SKIP with the reason "not supported by this
compiler (`expect: ice` needs kiln)". The corpus is
`tests/conformance/driver.crash/`.

### kiln-specific tests

A test with an `oven:` directive is kiln-specific. When the compiler is not
kiln, the test is a SKIP with the reason "not supported by this compiler
(kiln-specific `oven:` directive)".

When the compiler is kiln, its compile gets
`-fconstexpr-report=<scratch>/constexpr-report.jsonl` (a stale file is
removed first), and the report is read in the format of `spec/oven.md`
"Reporting protocol". Tests without `oven:` do not get the flag. Budgets are
checked only after the test kind's own checks pass, so a test that fails to
compile fails for that reason. Then:

- No readable report, or a malformed one: **FAIL** (kiln broke the protocol).
- `oven: <counter><=N ...` without a location bounds the report's `total`.
  Its `peak_bytes` and `peak_arena` are the maximum over the evaluations,
  `static_bytes` the static space's high-water mark over the TU (it covers
  unrecorded work too, spec/oven.md "Reporting protocol"),
  `evaluations` their count, and `splice_fast` the splice operands read
  without an evaluation (a total-only key: an evaluation line reads it as 0, so only
  an unlocated bound is meaningful).
- `oven: <line>[:<col>] <counter><=N ...` bounds the evaluations whose `loc`
  is in the test file at that line (and column, if given), combined as for
  the total (sums; maximum for `peak_bytes`, `peak_arena` and `static_bytes`; count for `evaluations`). If no
  evaluation matches, the test FAILs: a budget on nothing is a stale budget.
- Each bound is inclusive: a counter above a `<=` bound, or below a `>=`
  floor, is a **FAIL**, naming the location, the counter, its value and the
  bound. A floor states that something happened (`memo_hits>=1`: a call was
  answered from the memo table); `memo_hits` is summed like `steps`.

The report's total is listed with the commands under `-v`.

A test with an `inst-report:` directive is kiln-specific in the same way
(the SKIP reason names `inst-report:`). Its kiln compile gets
`-fkiln-instantiation-report=json`, which prints the template instantiation
report (`spec/sema-templates.md` §12) on stderr as NDJSON: one
`{"template":…}` row per template, then one `{"total":…}` row. The runner
takes exactly those lines from stderr, replaces the value of every
`"<name>_us":<digits>` field with `0` (wall-clock timings are not
reproducible), and compares the result line by line with the golden, a path
relative to the test's directory (blank golden lines ignored, golden timings
masked the same way). A mismatch is a **FAIL** showing both; a missing
golden is an **ERROR**. `--update` rewrites the golden from the masked
report. The check runs after the test kind's own checks pass. Row order is
the report's own (bytes descending, then count, then first instantiated),
so it is deterministic.

A test with a `written-refs:` directive is kiln-specific too (the SKIP
reason names `written-refs:`). Its kiln compile gets
`-fkiln-dump-written-refs`, which prints the written-reference table
(spec/ast.md §18.6) on standard output, and the runner compares that output
exactly with the golden, a path relative to the test's directory. A
mismatch is a **FAIL** with a line diff; a missing golden is an **ERROR**;
`--update` rewrites the golden. The check runs after the test kind's own
checks pass. The PCH differential (`--pch-diff`) checks no dump.

A `decl-extents:` test is checked the same way with
`-fkiln-dump-decl-extents`, which prints the declaration-extent table
(spec/ast.md §19.7); the SKIP reason names `decl-extents:`. With
`unordered` after the golden's path, the dump and the golden are compared
as their sorted lines, and `--update` leaves a golden whose sorted lines
match alone (so it can share the golden of a test that compares in
order) and otherwise writes the dump sorted. A test takes
one of the two dump directives, not both. The PCH differential checks no
dump here either (the dump flag is single-pass, spec/ast.md §19.6; the
extents' own PCH and module-image round trips are Rust tests).

A test with a `pp-record:` directive is kiln-specific too (the SKIP
reason names `pp-record:`). After the test kind's own checks pass, the
runner preprocesses the test once more, with the test's own options plus
`-E -fkiln-dump-pp-record`, and compares the standard output exactly with
the golden, a path relative to the test's directory: one `tok` line per
output token at a macro location, then the record's entries
(spec/pp.md "Preprocessing record"; WP-2110). A crash is a **FAIL**, a
mismatch is a **FAIL** with a line diff, a missing golden is an
**ERROR**; `--update` rewrites the golden. The PCH differential and the
header-differential mode check no dump.

### Module-file kinds

`--bmi-kind N` (`N` = 1 or 2) checks the module file reader under each
kind (spec/modules.md §11.11). The selection keeps only the test files that
contain `//@ module:` or `//@ header-unit:` (after `--filter`, before
`--shard`; the runner also SKIPs any other test, with the reason "no
`module:` or `header-unit:` lines (`--bmi-kind`)"; an `as-header-unit`
test without them is not selected). Every remaining test runs two-step, as if it said
`two-step` ("Directives"), and each of kiln's module-file builds gets
`-fkiln-bmi-kind=N` after the test's `args`. The oracle side is unchanged.
A `module:` unit that carries no constant result is written as kind 1
under either value (the `-v` note says so). The run therefore checks that
both kinds read back to the same diagnostics, program output and IR as the
single-step run. `ci` runs it twice (below). The IR goldens of
`tests/codegen` with `module:` lines are already two-step through
`ir_vs_clang` (spec/codegen.md §6), and they run under the default kind.

`--bmi-image` adds `-fkiln-bmi-image` to the same kind-2 run: each of
kiln's module-file builds and each kiln compile that is given
`-fmodule-file=` gets it (never the oracle, never a single-step source
build), so a unit inside the image's coverage (spec/modules.md §11.11
"Part 2a") is written with an image and imported from it, and every other
unit falls back to replay as before. The run therefore checks that
image-built imports give the same diagnostics, program output and IR as
replay. `ci` runs it after the two kinds (below; WP-1828).

### PCH differential

`--pch-diff <how>` (WP-1036) checks that `-include-pch` behaves like the
textual inclusion it replaces (spec/driver.md "Precompiled headers"). A
test runs when it is a C++ source file of kind `compile-ok`,
`compile-fail`, `run`, `ir` or `no-crash`; has no `pch:`, `unit:` or
`header-unit:` lines and no `args:` that select an `-include`, `-E`, `-M`
or `-x`; and opens with `#include <...>`/`#include "..."` lines (blank
lines and comments between them allowed; the prefix ends at the first
other line, a `\`-continued line, or a line with an inline `/*`).
Every other test is a SKIP naming the reason. `xfail` lines are ignored
and nothing is compared with the oracle (none runs).

kiln compiles a copy of the test in the scratch directory
(`src/<file>`, with `-iquote <test dir>` so its quoted `#include`s are
found) twice:

- **plain**: the copy as written.
- **with the PCH**: the copy with its leading `#include` lines emptied
  (every other line keeps its number, as a PCH user drops the includes it
  holds; an unguarded header would otherwise be read twice), and
  `-include-pch pchdiff.pch` plus the consumer flags: `replay`
  `-fno-kiln-pch-snapshot`, `snapshot` none (the default path since
  WP-1404: the consumer restores the snapshot). The modes `load` and
  `check` read the declaration image WP-1397 retired, and are now a
  usage error.
  `pchdiff.pch` is built from `pchdiff.h`, the leading `#include` lines,
  with the test's `-std`, `args`, the `-fmodule-file=` flags of its
  `module:` lines, `-iquote <test dir>`, in the consumer's order, and
  `-x c++-header`; the `replay` producer adds `-fno-kiln-pch-snapshot`,
  the `snapshot` producer adds nothing (the default path, with
  `KILN_SNAP_TIMING=1`, single pass where it may) and its
  `-fclang-compat=` and `kiln_meta.h` flags come first, as in the
  consumer: a snapshot is used only when the producer's command line
  is the consumer's, spec/pch.md "Identity"). Neither build passes the report
  options an `oven:`, `inst-report:`, `written-refs:`, `decl-extents:` or `pp-record:` test otherwise
  gets: no budget or dump is checked, and the option would be the
  consumer's alone (a producer without `-fkiln-written-refs` carries no
  written-reference table, and a consumer that records lists the prefix
  as unrecorded, spec/ast.md §18.10.3).

**The synthetic prefix** (`--pch-diff-synthetic`, WP-1395). A test of a
compared kind with no `#include` (or `#import`) line anywhere, which is
otherwise skipped, is given the prefix `#include <bits/stdc++.h>` (all of
libstdc++, the largest realistic prefix): the reference build is the copy
compiled with `-include pchdiff.h` (that one line), and it must give the
plain build's success, diagnostics (as compared below) and run, or the
test is a SKIP ("the synthetic prefix changes the test's result"; most
often a name the test declares that the library declares too, or a
diagnostic count). It is also a SKIP when the reference build or the PCH build
exceeds the test's memory cap ("the synthetic prefix exceeds the test's memory
limit"), and when the plain build reports `stack exhausted` ("the test
measures the stack depth, which the prefix shifts": where a stack guard
fires depends on how deep the compiler's own frames are when the main
file starts, which a restore changes by a few frames). The PCH build compiles the unchanged copy with
`-include-pch` of it, without `-iquote`. The prefix's PCH is built once
per distinct producer command line in the run and shared by its tests.
The comparison is then the reference against the PCH build. Not in `ci`:
over the corpus it costs about an hour of CPU.

**Snapshot accounting** (`snapshot`, WP-1395). Each consumer runs with
`KILN_SNAP_TIMING=1`, so kiln prints its snapshot notes as
`snapshot-timing: note: ...` and `snapshot-timing: restore ...` after a
restore. A test's reason is `snapshot` (restored), `replay: producer:
<why>` (the producer's `-v` note: the PCH has no snapshot) or `replay:
consumer: <why>` (the consumer's last note: the snapshot was not used).
A consumer that refused the snapshot for its configuration also warns
(`-Wpch-snapshot-unused`, WP-1403): the warning gives the reason when
there is no note, and it is not a diagnostic difference (it is dropped
before the comparison, as the notes are).
A FAIL keeps the reason too. With `--pch-diff-require-restore`, a test
whose consumer replayed FAILs unless a line of
`tests/pch-snapshot-replay-allow.txt` allows it: `<pattern> ::
<justification>`, both non-empty, `#` comments; the pattern is a
substring of the reason after `replay: `. A malformed or missing file is
an infrastructure error.

**A consumer with another search path** (`--pch-diff-extra-dir`,
WP-1403). Every other test's consumer (the hash of its name is even) gets
`-I <scratch>/consumer-only`, an empty directory the producer does not
have, after its other options. The snapshot must restore all the same
(spec/pch.md "Lookup-tolerant search path"): a restored consumer
whose `snapshot-timing: find` line does not say `lookups repeated` is a
FAIL, and so is a replay under `--pch-diff-require-restore`. The other
half keeps the fast path (equal search lists).

**The accounting lines.** After the summary, every `--pch-diff` run
prints `pch-diff <how>: N consumers ran[: R restored, P replayed]; F
failed before the consumer ran; S skipped`, then one line per path (the
reason above, or `image (N records)` / `replay: <why>` for `load`) with
its count and how many of those FAILed, and `skipped:` with one line per
skip reason (numbers reduced to `N`; a replay reason's quoted paths to
`'…'`). Under `snapshot` a last line counts the producers by shape,
`pch-diff snapshot producers: N built: S single pass, T two passes`,
then one line per shape (`single pass`, or `two passes (<why>)` with
the producer's reason, from its `snapshot-timing: producer single
pass:` line).

Each build is `-S -emit-llvm` and, for a `run` test, first the binary and
its run. A test with `module:` lines runs two-step (its units are built
separately, without the PCH: kiln's in-process build of a unit reads the
importer's `-include-pch`, as it reads its `-include`s, and anything before
`module;` is an error). The test **PASSes** when the two builds agree on:
success or failure; the diagnostics, after dropping the include stack
(`In file included from` lines), the `N warnings generated.` line and
kiln's `note: precompiled header` notes, either equal or equal to the
producer's diagnostics followed by the consumer's (a restored snapshot's
consumer does not diagnose the header again, and says how many the header reported, which
are dropped from the front of the plain build's); the IR after the usual
normalization ("IR tests"), and without DIFile checksums (the copy's
bytes differ) and inline asm `!srcloc` values (raw locations, which a
PCH shifts in clang too); and a run test's exit status, stdout and
stderr, with the binary's name, a sanitizer report's process id,
addresses and `BuildId` reduced (they differ between any two runs). An
`ir` test needs no oracle here. Its reason is the mode (`replay` or `snapshot`, with the producer's
shape and, for a refused snapshot, why). A
plain build that fails is a SKIP ("kiln fails without a PCH"), and so is a
PCH that does not build when the plain build fails too; a PCH that does
not build when the test compiles, a crash, or any difference is a
**FAIL** showing it.

`ci` runs `replay` and `snapshot --pch-diff-require-restore
--pch-diff-extra-dir`
over `tests/conformance/cpp.include` and
`tests/conformance/module.interface` (run 6 under "CI"). `cargo xtask
libcxx --pch-diff <how>` is the same comparison over libc++ tests
(spec/libcxx.md).

### Parity report

`--report-parity <file>` writes, after the run, a markdown summary between
the lines `<!-- parity:begin -->` and `<!-- parity:end -->` in `<file>`:
the compiler, the oracle, the pp comparison mode, the run date, and one
table row per directory of tests (the test's parent directory) plus a total,
with columns tests, pass, fail, xfail, xpass, skip, error and pass rate
(pass / (tests - skip)). Text outside the markers is kept, so the file can
carry hand-written context. A file without the markers gets them appended; a
missing file is created; a file with only one of the markers is an
infrastructure error (exit 2). The report is written whatever the outcome.
`docs/status/pp-parity.md` is the preprocessor parity page:

```
cargo xtask test tests/differential/pp --report-parity docs/status/pp-parity.md
```

## Generated pp corpus

`cargo xtask gen-pp-corpus` (`xtask/src/gen_pp.rs`) writes one `pp` test
per header to `tests/differential/pp/`, each `#include`-ing the header with
`-stdlib=libc++ -resource-dir %{resource_dir}`, oracle clang, and no golden:

| Directory | Headers |
|---|---|
| `libcxx/` | libc++'s public headers (files in its include dir without `.h`, plus `cxxabi.h`) |
| `libcxx/experimental/` | libc++'s public `experimental/` headers |
| `libcxx-c/` | the C headers libc++ wraps (its `.h` files) |
| `sysroot/` | top-level `<sysroot>/*.h` headers that libc++ does not wrap |

Excluded, with the reason printed by the generator: names starting with `__`
(internal), `ext/` (deprecated GNU extensions), `libcxx.imp` and
`module.modulemap` (not headers), and sysroot headers that cannot be
included on their own (`gnumake.h`, `regexp.h`).

Options: `--std <std>` (default `c++26`; any other standard goes to sibling
files named `<header>.<std>.cpp`), `--libcxx <dir>` (default
`/usr/lib/llvm-N/include/c++/v1`, N from `KILN_ORACLE`), `--sysroot <dir>` (default
`/usr/include`), `--out <dir>`, and `--check` (write nothing; exit 1 if any
file would change). The generator writes only changed files, removes stale
generated files (those starting with its banner) in the directories it owns,
and prints per-directory counts. Hand-written tests live in
`tests/differential/pp/torture/`, which the generator does not touch.

## Stress suites

`cargo xtask stress [test options]` runs `cargo xtask test` (same options,
same outcomes and exit status) over the suites too slow for `test` and `ci`.
They live outside the default roots, so neither runs them. Run `stress`
before a release candidate, and after a change to header-heavy Sema paths.

| Suite | Contents | Cost |
|---|---|---|
| `tests/header-guard/` | the libc++ header no-crash guard: one `expect: no-crash` test per libc++ header, in `libcxx/`, `libcxx/experimental/` and `libcxx-c/` (the pp corpus's directories, minus `sysroot/`), each with `-stdlib=libc++ -ferror-limit=0` and `mem<=2048` | 131 tests, about 75 s at `-j 2` (debug kiln, 2026-11): over the ~60 s budget for `ci` |

`cargo xtask gen-pp-corpus --no-crash` generates the guard. It uses the
pp generator's header enumeration and exclusions and accepts `--std`,
`--libcxx`, `--out` and `--check`. Any number of errors is fine. A panic, a
signal, an internal-error exit, a hang, or going over the memory budget is a
FAIL. It exists because the default error limit of 20 hid a reachable panic
in `<string>` (WP-141), so `-ferror-limit=0` is part of the point (WP-145).

## External corpora

`cargo xtask test --external <dir>` compiles every `.c`, `.cc`, `.cpp` and
`.cxx` file under `<dir>` (recursively, in path order) once with
`-std=<std> -fsyntax-only -ferror-limit=0 <args> <file>` (GCC:
`-fmax-errors=0` and the mapped args), taking `std:` and `args:` from the
file's directive header when it parses as one and `c++26` (`c17`) with no
args otherwise, with the file's directory as the working directory, on
`--jobs` threads, under `--mem-limit` (plus kiln's stack reservation), 120 s
each. The semantics are **never-crash**: a case passes when it compiles
(exit 0) or is rejected cleanly (exit 1); a signal, a panic, an `internal
compiler error`, a stack overflow, any other exit status, a timeout or a
memory kill fails the run. Expected diagnostics are not checked.

`<dir>` must lie outside the repository (after resolving symlinks; inside
is a usage error, exit 2): it is for corpora that may not be published,
such as crash reproducers reduced from private code. Nothing from it is
written anywhere; only counts are printed,

```
external: ok.|FAILED. N cases: A accepted, R rejected cleanly, C crashed, T timed out, K killed in S.SSs
```

and, with `-v`, the failing cases' paths relative to `<dir>`. Exit status
0 when no case failed, 1 when one did, 2 for an error. `cargo xtask bench
--external` is the same run, measured (spec/bench.md "External corpora").

## GCC testsuites

`cargo xtask gcc-torture` (WP-1277) runs GCC's own C torture tests
through kiln's C driver, GCC 16.1 and clang-19, case by case, and records
how each compiler did. It is informational, like `clang-neg` and
`libstdcxx`: never in `ci`, exit status non-zero only for a tool error
(kiln did not build, an oracle could not be started, an output could not
be written), never for a failing test.

**Source.** The tests are GPL and are never vendored.
`scripts/fetch-gcc-tests.sh [<tag>] [<dir>]` makes a shallow,
blob-filtered sparse checkout of `gcc/testsuite/gcc.c-torture` (plus the
seven `gcc.dg` / `gcc.target` files torture tests `#include` by relative
path) at `releases/gcc-16.1.0` (the GCC oracle's release) into
`.oss/gcc-tests` (gitignored, about 15 MB). Each run calls the script,
which fetches the checkout when it is missing and otherwise only refreshes
its sparse set (a failed refresh of an existing checkout is a warning);
`--tests <checkout>` points at another one, used as is. Nothing from the tests is
written into the tree; the outputs go to the target directory, and only
counts, test ids and one-line diagnostics reach a status page.

**Suites** (`--suite`, default `execute,ieee,builtins`; `compile` is
opt-in), as GCC's `.exp` drivers run them:

- `execute`: `execute/*.c`, gcc-dg-runtest, default action `run`, `-w`.
- `ieee`: `execute/ieee/*.c`, c-torture-execute, `-fno-inline`, `.x` files.
- `builtins`: `execute/builtins/*.c` (not `*-lib.c`), each linked with
  its `<test>-lib.c` when present and `lib/main.c`, with GCC's builtins
  flags (`-fno-tree-dse -fno-tree-loop-distribute-patterns -fno-tracer
  -fno-ipa-ra -fno-inline-functions`), `.x` files.
- `compile`: `compile/*.c`, gcc-dg-runtest, default action `assemble`, `-w`.

**Cases.** One case per test and option set (`--opt`, default
`-O0,-O2`). The compile flags are, in order, `-std=<std>` (`--std`,
default `gnu23`, GCC 16's default; `none` omits it), the option set, then
the test's flags. For the dg suites those are `dg-options` (replacing the
suite's `-w` default) and `dg-additional-options`, each applied only when
its target selector matches. For the c-torture suites they are the `.x`
file's `additional_flags` (a `set additional_flags` replaces the suite's
flags, as in GCC) and then `-w`. Every source is compiled with `-E`, `-S`
or `-c` by the action (`preprocess`, `compile`, `assemble`, `link`,
`run`), the objects are linked with `-lm`, and for `run` the executable
runs in the case's scratch directory. kiln's flags start with
`-fclang-compat=<major>` when the clang oracle's major version is one kiln
emulates (`oracle::SUPPORTED`), so kiln's defaults (clang 22's
`-Wincompatible-pointer-types` error, WP-1285) follow the oracle it is
compared with; the compiler line in the summary shows the flag.

**Directives.** `dg-do <action> [{ target|xfail <sel> }]` sets the action
(a non-matching target skips the case); `dg-skip-if "<why>" { <sel> } {
<include> } { <exclude> }` skips when the selector matches, every include
option is among the case's flags (`*` matches anything) and no exclude
option is; `dg-require-effective-target <name> [{ target <sel> }]` skips
when the name is false; `dg-xfail-if` / `dg-xfail-run-if` / an `xfail`
selector mark the case xfail (a column, not a pass: an xfail case still
gets its real class); `dg-timeout-factor` scales both timeouts;
an applicable `dg-error` makes the case negative (it passes when the
compile is rejected; the message is not matched). A `.x` file is evaluated
by a small Tcl subset (`set`, `lappend`, `if`/`elseif`/`else`, `return`,
`continue`, `global`, `load_lib`, with `istarget`,
`check_effective_target_*` and `string match` as substitutions and
short-circuit `&&`/`||`): `return 1` skips the test,
`torture_execute_xfail` on a matching target marks it xfail,
`torture_eval_before_compile` is re-run per option set with `option`
bound and its `continue` skips that option set, and anything it cannot
evaluate is a skip reason (`.x: <error>`), so an unsupported construct is
counted rather than guessed.

**Effective targets** (`xtask/src/gcc_tests/target.rs`) describe a hosted
LP64 GNU/Linux target (aarch64 or x86_64, glibc, PIE) identically for all
three compilers: the point is the same cases through each, not per-compiler
probing. `__OPTIMIZE__` follows the option set (true unless the last `-O`
is `-O0`); `run_expensive_tests` follows `GCC_TEST_RUN_EXPENSIVE`. A name
the table does not know skips the case as `unknown effective target <n>`,
so a new name shows up in the skip list.

**Flags a compiler rejects.** Each compiler probes every flag unit
(`--param x=y`, `-include f` and the like are one unit) once against a
trivial TU; a unit it rejects is dropped from its cases and listed in the
TSV's `dropped` column and the summary's dropped-flags section, so a GCC
`-fno-tree-*` flag does not turn every clang or kiln case into a reject.

**Classes**, per compiler and case: `pass`; `compile-reject` (a valid test
the compile or link step rejected; detail: the first error without its
location); `crash` (an internal compiler error, a panic, a signal or
non-standard exit status from the compiler, or the memory cap); `wrong`
(the program exited non-zero or died on a signal: GCC's tests `abort()`
on a wrong answer); `timeout` (compile, link or run outran
`--timeout` / `--run-timeout`, defaults 120 s and 10 s); `accept-invalid`
(a negative test compiled clean); `not-run` (the compiler could not be
started); the g++.dg runner adds `wrong-line` and `expect-unknown` ("The
g++.dg runner"). Each step runs under `--mem-limit` (default 2048 MiB address
space, plus kiln's stack reservation for kiln) and holds a jobserver token;
a step that cannot be spawned is retried twice, 2 s apart, before it is
`not-run`.

**Why `gnu23`.** GCC 16 defaults to `-std=gnu23` and its own torture runs
pass no `-std`, so the default keeps GCC's view of the tests. clang-19
rejects old-style (K&R) definitions and more implicit-int forms under C23
than under `gnu17` (on aarch64, clang-19 passes about 6800 of 7572 cases at
`gnu23` and 7057 at `gnu17`); those cases show as "kiln fails, clang
fails too" and say nothing about kiln-only gaps. `--std gnu17` measures
the older dialect.

**Outputs.** `--tsv` (default `<target>/xtask-gcc-torture/results.tsv`):
one row per case, columns `test opt kiln kiln_detail gcc gcc_detail clang
clang_detail xfail dropped` (`-` for an oracle not run; skipped cases read
`skip` and the reason, collapsed to one `*` row when every option set skips
alike). `--summary` (default `.../summary.md`): the run's header
(commits, compilers, flags, limits, wall time), results by class per
compiler, kiln's pass rate by suite and option set, kiln against the
oracles (passes both pass and kiln does not, and the reverse), kiln's
failure clusters (class plus detail with quoted names and numbers folded,
top 60 with example tests), skip reasons and dropped flags. `--report
<page>` splices the same region between `<!-- gcc-torture:begin ... -->`
and `<!-- gcc-torture:end -->` in a status page, keeping the hand-written
triage around it. A run at a `--std` other than the default gnu23 has its
own region, `<!-- gcc-torture:begin std=<std> ... -->` to
`<!-- gcc-torture:end std=<std> -->` (`std=none` for `--std none`), so one
page holds runs at several standards (WP-1282). `--list` prints the planned cases and skips and runs
nothing.

**Reuse.** The pieces under `xtask/src/gcc_tests/` are suite-independent:
the checkout (`locate`), the Tcl list subset (`tcl`), directive extraction
and per-option resolution (`dg`), selectors and effective targets
(`target`), `.x` evaluation (`xfile`), a compiler with flag probing and
the compile/link/run step (`Tool`, `Case`), and the classes and cluster
keys (`outcome`). `cargo xtask gxx-dg` (below) adds its suites, the
standard list, the diagnostic matcher (`expect`) and its report on top.

### The g++.dg runner

`cargo xtask gxx-dg` (WP-1443) runs GCC's C++ tests through kiln++, GCC
16.1 and clang, informational like `gcc-torture` (never in `ci`; exit
status non-zero only for a tool error). The checkout is the same one
(`scripts/fetch-gcc-tests.sh` adds `g++.dg`, `c-c++-common`,
`g++.old-deja` and the nine `gcc.dg`/`gcc.target` files their tests include
by relative path; about 160 MB with `.git`).

**Suites** (`--suite`, default all four), as GCC's `.exp` drivers run them:

- `dg`: `g++.dg/**/*.C` in every directory without an `.exp` file of its
  own (`find-cxx-tests` of `g++.dg/dg.exp`; `g++.dg/torture` is its own
  suite), default flags `-pedantic-errors -Wno-long-long`. The directories
  with their own driver (`modules`, `lto`, `gomp`, `asan`, `pch`, ... ) are
  not run; the summary lists them with their file counts.
- `torture`: `g++.dg/torture/*.C` and `c-c++-common/torture/*.c`
  (`dg-torture.exp`: gcc-dg-runtest, no default flags, no `-std=`), at the
  option sets `--opt` (default `-O0,-O2`, the two of GCC's torture list
  this runner uses).
- `common`: `c-c++-common/*.[cSi]` and `c-c++-common/cpp/*.[cS]`, no
  default flags; a `.c` file is compiled as C++ (`-x c++`, as `g++` does).
  `.S` and `.i` inputs are skipped as `input kind`.
- `old-deja`: `g++.old-deja/**/*.C`, default flags as `dg`.

The default action is `compile` (`-S`, as GCC's); `dg-options` replaces a
suite's default flags.

**Standards.** Each test of `dg`, `common` and `old-deja` runs at the
standards `g++-std-flags` of GCC 16's `lib/g++-dg.exp` picks: one run with
no `-std=` added when the test names `-std=*++` anywhere; else the prefix
is `-std=gnu++` when the test has a `dg-options` line and `-std=c++`
otherwise, and the years are those named on the test's `{ dg* c++NN` lines
(a year on a `dg-do` or `dg-require-effective-target` line other than 98
is also the floor; `coroutines/` has floor 20, `modules/` 17), filled to
three from `20, 98, 26, 17, 11, 14, 23` above the floor (98 counts as 3).
A plain test therefore runs at C++20, C++98 and C++26. `--std 20,26` is
GCC's `GXX_TESTSUITE_STDS`: that list, filtered by the floor. A case
without any `-std=` (the torture suite, a test whose own `-std=` sits under
a selector that does not match) gets `-std=gnu++20` first, GCC 16's
`cxx_default`, so that kiln and clang compile at GCC's default too. The
case's standard (the last `-std=` of the run's flags or the test's options)
answers the `c++NN` (NN or later), `c++NN_only`, `c++NN_down` (NN or
earlier), `c++` and `c` effective targets (`target.rs`).

**Directives** beyond the torture runner's: `dg-additional-sources
"<files>" [{ target <sel> } | linkonly]` (`lib/gcc-defs.exp`; WP-1664):
each word a file relative to the test's directory, every directive
whose selector matches, a `linkonly` one only when the case links; each
source is compiled with the test's flags and the objects are linked
together for `dg-do link` and `run`. For `dg-do compile` or `assemble`
each source is compiled on its own (DejaGnu hands them all to one driver
run with `-o`, which GCC rejects; no test in the tree outside a
`linkonly` source does this). An `xfail` or unknown selector skips the
case with the reason, a source that is not there skips it as `missing
source <name>`; directives in an additional source are not read
(DejaGnu reads only the test's). `dg-shouldfail` (a run
test passes when the program fails and is `wrong` when it exits 0);
`dg-ice` marks the case xfail; any `dg-module-*` skips the test as
`modules (dg-module-*)`, counted, not run. `dg-final` checks
(`scan-assembler`, `scan-tree-dump`, ...) and `dg-output` are GCC-internal
or output checks the runner does not compare: the case's class comes from
its compile (and run), and the summary counts the tests holding each kind
under "Not applicable". Flags a compiler rejects are dropped for it as in
the torture runner, each C++ tool probing a unit after the case's `-std=`
(GCC 16 takes `-freflection` only at C++26); a unit is a flag and, for
`-D`, `-U`, `-I`, `-isystem`, `-idirafter`, `-iquote`, `-include`,
`-imacros`, `--embed-directory`, `--param`, `-Xlinker` and `-mllvm`, the
word after it. In a test's options (the Tcl evaluation of the directive)
`${srcdir}` / `$srcdir` is the checkout's `gcc/testsuite` and `${subdir}`
/ `$subdir` the directory of the driver under it (`g++.dg` for `dg` and
`common`, `g++.dg/torture`, `g++.old-deja`). A unit that is a single
word, not a flag, with a C, C++ or object extension (`.c`, `.C`, `.cc`,
`.cpp`, `.cxx`, `.c++`, `.o`) is an input file the driver compiles with
the test (`dg-options "-O3 ${srcdir}/g++.dg/x/y-1.C"`): it joins the
sources after the test instead of being probed as a flag.

**Diagnostic matching** (`xtask/src/gcc_tests/expect.rs`). The
expectations of a case are its applicable `dg-error`, `dg-warning`,
`dg-message` and `dg-bogus` directives, `dg-<kind> <regexp> [<comment>
[{ target|xfail <sel> } [<line>]]]`: a non-matching `target` drops one, a
matching `xfail` marks it (GCC does not meet it); the line is the
directive's own, or `.`, `.-N`, `.+N`, a number, or 0 for "no line". A
compiler's diagnostics are its `<file>:<line>[:<col>]: <severity>:` lines;
`error`, `fatal error` and `sorry, unimplemented` are errors. The file is
not looked at, as `DejaGnu` does not (a test expects an error in a header
it includes by that header's line); a diagnostic without a location has
line 0. Only the line and the severity are compared, never the message (GCC's
regexps are GCC's wording; kiln's is clang's). With `required` the lines of
the non-xfail `dg-error`s:

- no line required: the case passes when the compiler reports no error and
  every step succeeds, else it is `compile-reject` (rejects-valid);
- lines required, no error at all: `accept-invalid` (detail: the first
  expected regexp);
- errors reported: every required line other than 0 must hold an error
  (else `wrong-line`, `missed error at L<n>`), and no error may sit on a
  line where nothing is expected (else `wrong-line`, `extra error at
  L<n>: <first such error>`). An error where a `dg-warning` or
  `dg-message` is expected is accepted (`-pedantic-errors` and
  `-Werror=` make severities differ between compilers) and so is an error
  on an xfail `dg-error` or xfail `dg-bogus` line (GCC errs there); one on
  a `dg-bogus` line is extra. A line argument may be a `dg-line` name. Errors
  with line 0 satisfy a line-0 expectation and are never extra; a line-0
  `dg-error` or `dg-message` (DejaGnu matches it on every line of the
  output) accounts for an error on any line.
  `dg-excess-errors`, or a `dg-prune-output` that may match an error
  line (below; its pattern is GCC's wording, which cannot be applied to
  other compilers' output), turns the extra-error check off for the case.

**Severities the file names** (WP-1554, from GCC 16.1's `lib/gcc-dg.exp`,
`lib/gcc-defs.exp`, `lib/prune.exp` and DejaGnu's `dg.exp`). `dg-error`'s
regexp gets the prefix `(fatal )?error:`, `dg-warning`'s `warning:`;
`dg-message` gets none (it is `dg-warning` without a prefix), so its
regexp matches a diagnostic of any kind on its line, and `""` matches any
diagnostic there (at line 0, any line of the output). `dg-regexp` is a
regexp over the whole output, with no line, that must match; `dg-prune-output`
removes what its regexp matches before the excess-errors check;
`dg-excess-errors` marks that check xfail; a compile test is judged by the
output alone, never by the exit status. The runner reads a regexp only for
the severity it names: `error:` (or `fatal error:`) or `sorry,
unimplemented` names an error; `warning:` or `note:` (and no error) names
another kind; anything else, `""` included, names none. So:

- a `dg-message` naming an error is a `dg-error` (required on its line);
  one naming another kind is a message as before; a non-xfail one naming
  none leaves the kind open on its line;
- a `dg-regexp` naming an error requires an error with no line (any
  line); one naming another kind expects nothing the runner checks; one
  naming none may hold errors anywhere;
- `dg-excess-errors`, and a `dg-prune-output` that may match an error
  line, may hold errors anywhere.

**Which prunes may match an error line** (WP-1617). `lib/gcc-dg.exp`
removes every output line holding a match of the pattern (`[^\n]*<p>[^\n]*`)
before the excess-errors check; pruning never makes an expected error. A
GCC error line is `<file>:<line>:<col>: error: <text>` with any text, so
the runner decides from the pattern alone, and a prune that cannot match
one changes nothing (no extra-error exemption, no `expect-unknown`). It
cannot when all of these hold: the word as Tcl hands it over (braced:
as written; quoted or bare: `\n`, `\t`, `\r` and `\X` for punctuation
X substituted) has no command or variable substitution (`[`, `$`) and
no other letter escape; the regexp holds no `|`, no `error`, no `sorry`,
no `(?` and no `***` director, and its groups and bracket expressions
balance; and outside groups and bracket expressions it holds `warning:`
or `note:` as a run of literal characters (`\X` for punctuation X is
literal; `.`, `^`, `$`, an escape class, a group or a bracket expression
ends a run; a character under `*`, `?` or a bound `{...}` is not in a
run, and one under `+` ends it). That a GCC error's text never holds
`warning:` or `note:` is the one assumption, the same the severity
reading of `dg-message` makes. Every other pattern may match an error
line: `"note"`, `"warning"` and `"In function"` (the word may sit in an
error's text), `"note:|x"`, `"(note: )?x"`, `"note:?"`, an empty
pattern.

When no error is required and the compiler reports errors (or fails
without an error line), the case is `expect-unknown` instead of
`compile-reject` if the file does not decide it: the case does not link
(a `dg-do link` or `run` test must compile under GCC, so there an error
nothing requires is always a reject), and either a directive above may
hold errors anywhere or every reported error sits on a line whose
`dg-message` leaves the kind open. The detail names the directive (and
its line) before the first error. `expect-unknown` is neither a pass nor
a failure: it is counted apart (the headline's pass rate is over the
decided cases), has no triage bucket, and the oracles run on it as on a
failure; GCC's result there is evidence for the triage, never the rule.

**Expected link errors** (WP-1664). A case that links and has a
non-xfail `dg-error`, or a `dg-message` naming an error or no severity,
at line 0 (`// { dg-message "i" "" { target *-*-* } 0 }` in a `dg-do
link` test: GCC's linker output, "undefined reference to ...", is what
meets it) expects a link error when its compile reports no error and no
diagnostic without a line (such a diagnostic could meet the directive
itself, and the case is judged as before). The line-0 directive is then
left to the link: a link that fails (`compile-reject` of the link step)
is the pass, and a link that succeeds is `accept-invalid` (`linked
(expected a link error: <directive>)`), the classes of the compile's
"error expected, none" and "error, none expected"; a link that fails in
a case without such a directive stays `compile-reject` (`link: ...`).

Missing warnings and messages are not failures. A negative case that passes stops after
the compile; a positive one goes on to link and run as its action says,
classed as in the torture runner (`wrong`, `crash`, `timeout`).

**Oracles.** kiln runs every case; GCC (`--gcc`, default `$KILN_GCC`) and
clang (`--clang`, default `clang++-22`, which also makes kiln's flags start
with `-fclang-compat=22`) run only the cases kiln does not pass
(`--oracles-on fail`, the default; `all` runs them on every case), through
the same judge; GCC with `-fdiagnostics-plain-output` and `LC_ALL=C`, as
GCC's testsuite runs it. kiln and clang get `-ferror-limit=0`: GCC has
no error limit, and an expected error after clang's default 20th would be
lost. When the GCC oracle runs, kiln and clang build
against its libstdc++ (`--gcc-toolchain=<its prefix>`: the headers the tests
were written for, `<meta>` among them) and every link adds that library
directory as run path (`-Wl,-rpath,...`), as GCC's own install does; the
report header notes the flag without the local path. A line that kiln
echoes from the source under a diagnostic (`  21 | ...`) never counts as an
internal compiler error, whatever its text. A GCC failure on a case is a runner or environment
question (or a GCC bug candidate where the test contradicts the standard),
never kiln's.

**Triage buckets** of a kiln failure: `kiln crash / hang` (crash or
timeout), `wrong-code` (a run test compiled and the program failed),
`rejects-valid` split into `clang accepts` (clang passes the case: a kiln
bug candidate) and `clang rejects too` (a GCC extension or GCC-specific
expectation, low priority), `accepts-invalid` split into `clang rejects`
(clang reports an error, on whatever line: a kiln bug candidate) and
`clang accepts too`, and `wrong-line` split into `clang matches` (clang
passes the case) and `clang differs too` (a location convention GCC and
clang do not share). Clusters inside a bucket use the
torture runner's cluster key.

**Outputs.** `--tsv` (default `<target>/xtask-gxx-dg/results.tsv`): one
row per case, `test opt kiln kiln_detail gcc gcc_detail clang clang_detail
negative xfail dropped` (`opt` is the case's `-std=`, `-` for none, or the
torture option set); while the run goes, each finished case is appended
to `<tsv stem>.partial.tsv` (removed at the end), so an interrupted run
keeps its rows. `--summary` (default `.../summary.md`): the header,
results by class per compiler, the buckets, the `expect-unknown` cases by
the directive that leaves them open, the priority directories
(`g++.dg/reflect`, `g++.dg/contracts`, `g++.dg/cpp26`, `g++.dg/cpp2a`,
`g++.dg/cpp23`, `g++.dg/torture`, `c-c++-common/torture`), every directory
by class, the top 30 clusters of each bucket with examples, and the not
applicable counts (skips by reason, unchecked `dg-final` kinds, directories
with their own driver) and dropped flags (each flag once, without the
standard it was probed under, absolute paths as `<path>`, the first 60
and a count of the rest). `--report <page>` splices it
between `<!-- gxx-dg:begin ... -->` and `<!-- gxx-dg:end -->`
(`docs/status/gxx-dg.md`). `--sample <n>` runs n tests evenly spaced over
the sorted selection, `--filter` selects by id globs (comma-separated), `--list` plans only.
Each step holds a jobserver token and runs under `--mem-limit` (default
2048 MiB plus kiln's stack reservation) and `--timeout` (default 60 s per
compile or link step; `--run-timeout` 10 s per run).

**Shards** (WP-1473). `--shard K/N` (1-based, as `cargo xtask libcxx`'s)
keeps every N-th test of the selection (after `--filter` and `--sample`),
starting at the K-th: round robin over tests, never over cases, so a
test's standards stay in one shard, the N shards are disjoint and cover
the selection (`gxx::tests::shards_partition_the_tests`), and each shard's
TSV holds its tests' skip rows too. The command line in a shard's summary
names its `--shard`. The x86_64 sweep (docs/VALIDATION.md "x86_64 sweep")
runs the suite as shards and merges their TSVs.

**Comparing runs** (WP-1473). `scripts/gxx-dg-compare.py` (standard
library only; `python3 -I scripts/gxx-dg-compare.py --help`) reads the
TSVs:

- `summary A.tsv`: the headline, "N of M applicable cases pass (P%)",
  where an applicable case is a row that is not a skip row; the class
  counts per compiler; kiln's failures by triage bucket (the runner's
  rules, re-derived from the classes in the row).
- `compare BEFORE.tsv AFTER.tsv [--labels B,A] [--list]`: both headlines,
  the class matrix of kiln over the cases (test, opt) both runs have (rows
  the first run's class, columns the second's, `skip` included), the
  counts of pass-to-failing, failing-to-pass and failing-to-other-failing
  cases, and every pass-to-failing case with its new detail and the
  oracles' classes (`--list`: the failing-to-pass cases too). It compares
  two runs of one host over time, and two hosts (aarch64 against
  x86_64) at one kiln commit.
- `clusters A.tsv [--bucket B] [--top N]`: kiln's failure clusters per
  bucket, by the runner's cluster key (`outcome::cluster_key`: class and
  first error with quoted names and digit runs folded), with test counts
  and examples.
- `merge --out M.tsv [--expect N] S1.tsv ...`: one run's shard TSVs into
  one, sorted by (test, opt); an error when the headers differ, a case
  appears in two shards or, with `--expect`, the shard count is not N.

## Resource limits

A runaway test must die alone. On 2026-10-31 one `kiln -fdump-ast` that
followed a cycle allocated without bound, and the kernel OOM-killed the
whole container (every agent on it) twice. So every process a test runs
has an **address-space cap** (`RLIMIT_AS`), on by default:

- The cap is `--mem-limit <MiB>` (default **8192**), lowered by the test's
  `mem<=N`: the smaller of the two. `--mem-limit 0` removes the default
  cap; a `mem<=N` budget still applies.
- It applies to every compiler and oracle invocation and to the `run`
  test's binaries (the compiler's and the oracle's). The timeouts are
  unchanged and independent.
- **kiln's stack reservation.** kiln reserves its compiling thread's stack
  as address space up front (1 GiB by default, `-fkiln-stack-size=N[K|M|G]`,
  at least 4 MiB; `spec/sema-templates.md` §10.3). For a program of kind
  kiln (the compiler under test, or the oracle) the cap of its invocations
  is the budget **plus** that reservation, taken from the command's last
  valid `-fkiln-stack-size=`, else 1024 MiB. A budget therefore measures
  what the compile allocates, and `mem<=512` is usable. Test binaries get
  the bare cap.
- **Mechanism.** `std::process::Command` has no rlimit API and the harness
  is `#![forbid(unsafe_code)]` (no `pre_exec`), so the command is run as
  `sh -c 'ulimit -v "$1" || exit 125; shift; exec "$@"' kiln-mem-limit <KiB>
  <program> <args...>`, with the original working directory and
  environment. `exec` keeps the pid, so timeouts, process-group kills and
  peak-RSS measurement are unaffected, and children inherit the cap. If the
  cap cannot be set (a lower hard limit), the wrapper exits 125 and the test
  is an **ERROR**.
- **Detecting a kill.** An allocation that fails under the cap does not
  produce a uniform signal, so a process that ended unsuccessfully (not
  timed out) counts as killed by the limit when its stderr shows an
  allocation failure:
  - when it was killed by a signal or exited with 128 or more: Rust's
    `memory allocation of N bytes failed`, an uncaught `std::bad_alloc`,
    or `LLVM ERROR: out of memory`;
  - any failure with a line starting with Python's `MemoryError`, the
    dynamic loader's `error while loading shared libraries: ... failed to
    map segment`, or the wrapper's `kiln-mem-limit: ... Cannot allocate
    memory`.
  The decision is `util::process::out_of_memory`. A process that fails
  otherwise under the cap is judged as usual (crash, wrong output, ...).
- **No core dumps for test binaries** (WP-1888). The `run` test's
  binaries (the compiler's and the oracle's) run under `sh -c 'ulimit -c
  0 2>/dev/null; exec "$@"' kiln-no-core <program> <args...>`
  (`util::process::without_core_dumps`, inside the cap's wrapper), so a
  binary that crashes is reported by its signal ("killed by signal N") at
  once. Writing a core, or a core-dump helper such as apport or
  systemd-coredump on a loaded hosted runner, had taken past the
  timeout, and the crash read as "timed out". The compiler and oracle
  compiles keep the host's core-dump setting.
- The outcome is **KILLED**, with the reason `memory limit <N> MiB` (the
  budget, without the stack allowance). The details name the process (the
  compiler, the oracle, the test binary, the oracle's binary), the cap and
  any stack allowance, and the last 20 lines of its stderr; `-v` logs each
  command with its `[mem<=<cap> MiB]`. KILLED is a failure and, like ERROR,
  is never masked by `xfail`: a memory blow-up is never an expected
  failure.

The run header names the default cap (`mem limit: <N> MiB` or `none`).
`cargo xtask bench` applies the same cap to its measured compiles
(`spec/bench.md` "Resource limits").

Bench budgets are the bench's own: `mem<=N` caps a measured compile's
address space as above, and `budget-time: <ms>` (spec/bench.md
"Directives") fails a bench entry whose fastest kiln run takes longer,
`--record` included, so a re-record cannot absorb a regression past it.

## Release gate

`cargo xtask release-gate` (WP-1219, `xtask/src/release_gate/`) runs the
gate every kiln tag needs (docs/RELEASING.md) as one resumable command
and writes its record. Its step manifest (`manifest.rs`) is the single
source of what a gate is; nothing else lists the steps.

**Steps.** Each step has an id, a host (`local`: the aarch64 development
host, run by `run`; `hosted`: GitHub's x86-64 runners, `sweep.yml`, read
back by `import-x86`), the existing command it calls, how its counts are
read, a pass rule, a **minimum unit count**, rough minutes and scratch
GB, and its dependencies:

| step | host | pass rule | minimum units |
|---|---|---|---:|
| `archive` | local | `dist --pgo --check` and `sha256sum -c` exit 0; the unpacked `kiln++ --version` names the version; `build-info.txt` is this commit's and says `pgo: yes` with at most 100 of kiln's functions without profile data (below) | 2 archives |
| `merge-gate` | local | `cargo xtask ci`: exit 0, every `corpus (...)` line `ok.` with 0 failed | 6000 (`corpus (kiln)` passed) |
| `oracle19` | local | `KILN_ORACLE=19 cargo xtask ci --oracle-only --step kiln`: the same | 6000 |
| `pch-sweep` | local | `scripts/pch-release-gate.sh <kiln++> <dir> all` prints `pch-release-gate: pass` | 4000 (libc++ pair tests) |
| `libcxx-aarch64`, `libcxx-x86_64` | local, hosted | PASS count not below the stored baseline | 9000 tests |
| `torture-gnu23`, `torture-gnu17` | local | `cargo xtask gcc-torture`: kiln's pass count not below the stored baseline | 7000 cases |
| `gxx-dg-aarch64`, `gxx-dg-x86_64` | local, hosted | by case against `docs/status/gxx-dg-baseline.txt` (below) | 50000 cases |
| `oss-<project>` | local | one per row of `scripts/oss/projects.toml`, the LLVM rows included: `report.json`'s TUs ok and tests passed not below the row's `docs/status/oss.md` section (kiln column) | 1 TU |
| `oss-x86_64` | hosted | every `sweep = true` row reported, each at its oss.md baseline, each count capped at what the same run's clang control reached on that host (a project can build fewer translation units on x86-64 than on the development host; WP-1718) | the sweep rows |
| `external-<id>` | local | `--external <id>=<command>`: exit 0; the command's last line is the step's result and holds `units=<n>` | 1 |

A step that ran fewer units than its minimum FAILS whatever its exit
status (a failed clone once gave an OSS row 0 of 0 translation units and
exit 0). A release gate requires the external `downstream` (the
record's "Downstream build" section); a gate without it is incomplete.
The stored count baselines are `docs/status/release-gate-baseline.tsv`
(`step`, `count`, value, source); the explained g++.dg cases are
`docs/status/gxx-dg-explained.txt` (`<test> [<opt>] <reason>`).

**The binary under test is the shipped kind** (WP-1862). The `archive`
step builds `cargo xtask dist --pgo --check`, the build `release.yml`
publishes, and every later local step that runs a compiler runs the
unpacked archive's `kiln++`: `pch-sweep`, `libcxx-aarch64`, the
torture steps (through the `kiln` beside it), `gxx-dg-aarch64`, every
`oss-<project>` and every `external-<id>`, which gets it as
`$KILN_GATE_CXX` (an external depends on `archive` and its record is tied
to the binary's hash like the others). `merge-gate` and `oracle19` are
the merge gate re-run at the RC: `cargo xtask ci` tests the checkout with
the debug build it makes. The archive's row and the record's summary
say how the binary was built, from `dist`'s `build-info.txt` ("Dist"):
`PGO build, profile sha256 <first 16 hex digits> (<n> training cases,
llvm-profdata <version>; <n> of kiln's functions without profile data)`,
with the counts `pgo=1` and `unprofiled=<n>`. The step fails when the
file is missing, names another commit, says `pgo: no`, or counts more
than 100 of kiln's functions without profile data (a profile that did
not apply; a matching one leaves 0, a mismatched one tens of thousands).
The hosted half is a PGO build too: a `sweep/rc-*` push builds `dist
--pgo` and uploads its `build-info.txt` with a `commit` file as the
artifact `kiln-build`, which `import-x86` reads; when it is missing or
fails the same checks, a release import records nothing (a smoke
import goes on), and each hosted row's detail ends with `kiln++:
<the same line>`. `--no-pgo` (every command) is the escape hatch for a
re-run: `archive` builds plain `dist --check` (the row says `plain
build (no PGO), --no-pgo`, `pgo=0`) and `import-x86` accepts a plain
hosted build.

**g++.dg by case.** The baseline holds kiln's class per case (test id,
option set) for each host, nothing else (`scripts/gxx-dg-compare.py
baseline`; any subcommand reads it as `FILE@HOST`). The step passes when
`gxx-dg-compare.py compare --gate --explained` finds no case that went
from pass to a failing class unless the explained list names it; the
pass percentage is reported, not gated. A full run's TSV is reused for
the verdict while its stamp (commit and `kiln++` hash) matches.

**Subcommands.** `plan` prints every step with its status (done with
its verdict, stale with the reason, missing), its rule, minimum, time
and scratch; no side effects. `run [--step <id>,..] [--from <id>]
[--redo] [--dry-run]` runs the missing and stale local steps in manifest
order (the archive first; then two tracks side by side, as the
hand-driven 1.2.1 gate ran them, all builds on the shared jobserver),
each with its output in `<state>/logs/<id>.log`, and prints the push
command for the hosted steps (it never pushes). It refuses a step whose
scratch need plus 50 GB exceeds the free space of the state directory's
file system, and a release gate on a tree with uncommitted changes.
`import-x86 <dir>` reads the artifacts of a `sweep/rc-*` run downloaded
into `<dir>` (one directory per artifact): every artifact it reads
carries a `commit` file, which must be this commit, or nothing is
recorded. `control <step> <unit> --previous <kiln++> [--times n]` runs
one unit (an OSS project, a libc++ test, a torture or g++.dg glob) n
times with the gate's and the previous release's `kiln++` and prints the
table; it decides nothing. `record [--out f] [--refresh-baselines]`
writes `docs/status/release-<version>.md`; `check <file>` verifies one.
Options: `--state <dir>` (default `<target>/release-gate`),
`--external <id>=<command>` (kept in `<state>/externals.tsv`),
`--smoke` and, with it only, `--kiln <kiln++>`; `--no-pgo` (above).
Every step gets a
driver named `kiln++` (kiln picks C or C++ by the name it is invoked
as): the path is made absolute without resolving symlinks, a `kiln`
stands for the `kiln++` beside it, and any other name is refused; the
same holds for `control --previous`. The command never
pushes, tags, publishes or deletes a branch, and deletes only inside its
state directory (an OSS row's build tree once its report is copied).

**State records.** `<state>/<id>.rec`, `key: value` lines: `step`,
`commit` (full), `version`, `mode` (`release` or `smoke`), `kiln` (the
FNV-1a 64 hash of the `kiln++` it ran, `-` for none), `start` and `end`
(Unix seconds), `load` (1-minute load at start and end), `arch`,
`counts` (`name=value` words), `units`, `verdict` (`pass` or `FAIL`),
`detail`, `log` and `log_hash`. A record is written when its step ends
(a killed step has none). A record is **stale**, and the step re-runs,
when it was made at another commit, in the other mode, for another
version, without counts, with another `kiln++` (steps that run the
archive's), or when its log is missing or changed. There is no option
that accepts a stale record: this is the no-carried-over-rows rule
(WP-1475). A failed record is complete; `--redo --step <id>` re-runs it.

**The record.** `record` fails, listing each, when a manifest step has no
completed record for this commit. Otherwise it writes one generated
region per section between `<!-- release-gate:begin <section>
commit=<short> version=<v> mode=<m> fnv64=<hash> -->` and `<!--
release-gate:end <section> -->` (sections `summary`, `archive`,
`merge-gate`, `pch`, `torture`, `libcxx`, `gxx-dg` (with the explained
list), `oss`, `external`), each a table with one row per step (step,
host, commit, units, counts, verdict, detail); text outside the markers
is kept. Only load, architecture and times reach it, no host path.
`--refresh-baselines`, after a passed release gate, rewrites the stored
baselines, the g++.dg baseline (from the two runs' TSVs) and empties the
explained list. `check` fails when a manifest step has no row, a row is
at another commit, a row's verdict is not `pass`, the sections disagree
on commit or version, a section is in `smoke` mode, or a region's hash
does not match its marker (an edit by hand).

**Smoke gates.** `--smoke` restricts the suite steps to one small
selection (gcc-torture `execute/20000*` at `-O0`, g++.dg
`g++.dg/cpp26/*`, libc++ `std/utilities/any`), applies the
smoke minimums and no stored baseline, and marks every record and the
record file (`mode=smoke`, a banner); such a record never passes
`check`.

## Jobserver

Every agent on the host shares one CPU/memory budget, so xtask is a GNU
make **jobserver client** (`xtask/src/jobs.rs`), like the cargo and rustc
it runs alongside:

- **What is honoured.** `MAKEFLAGS` containing `--jobserver-auth=fifo:<path>`
  (make 4.4's fifo style; the last such word wins). `scripts/env.sh` exports
  it when the host pool is live. The fifo is opened read+write; a token is
  one byte read from it, and the same byte is written back on release.
- **Scope.** Every child started through `util::process` (`run`,
  `run_with_input`, `run_measured`) holds one token for its whole lifetime:
  each compiler, oracle and test-binary process of `cargo xtask test`, each
  compile and run of `cargo xtask libcxx`, each compile of
  `cargo xtask libstdcxx`, each measured compile of
  `cargo xtask bench`, and the `--version` probes. Timeouts and the
  measured wall clock start once the token is held. Helper invocations of
  `git` and `cargo` are not counted (cargo is a client itself).
- **The implicit token.** As with make, the xtask process owns one token
  that is not in the pool: one child may run without reading the fifo;
  each further concurrent child needs a pool byte. So N concurrent xtask
  processes on a pool of P tokens run at most P + N children.
- **`-j`** stays the number of worker threads, so it is an upper bound on
  in-flight children; the pool can only lower the effective parallelism.
- **Release.** Tokens are released by `Drop`: on a normal exit, an error
  return, a worker panic, and a child killed by the memory cap or a timeout.
- **Signals (WP-157, WP-163, WP-434, WP-1436).** xtask catches **SIGINT and SIGTERM** on a
  dedicated thread (the `signal-hook` crate), with or without a pool. The
  thread starts with the first child. The handler's `unsafe` stays inside
  that dependency, so xtask remains `forbid(unsafe_code)`. On either signal:
  1. **The children die first.** Every child started through
     `util::process` (`run`, `run_with_input`, `run_measured`) is in a
     table from spawn to reap, and each is the leader of its own process
     group.
     - Each live group is killed with `SIGKILL`: the wrapper, GNU time, the
       compiler driver and `cc1` all go together.
     - The `cargo` steps of `cargo xtask ci` and the kiln build of
       `util::build_kiln` (every runner's first step) go through
       `util::process::run_step`, which puts them in the same table when the
       signal thread is running. Their group gets `SIGTERM` first, so cargo
       can stop its rustc and test children, and `SIGKILL` 3 s later if it
       is still alive. Cargo is a jobserver client of its own and holds no
       xtask token, so tokens that cargo still held when it died are lost
       until `cargo xtask jobserver refill`. The thread that ran the step
       then waits and leaves the exit to the signal thread (see "Always
       exits" below).
     - The thread then waits until each child is reaped and no spawn is in
       flight. It waits at most 10 s, for a child in uninterruptible sleep.
     - A child whose spawn was in flight is killed as it registers.
     - From then on, every new spawn fails with `Interrupted`, so a worker
       that sees its test die cannot start the next one.
  2. Every pool byte the process still holds (by live tokens, or already
     read for waiting workers) is written back to the fifo. This happens
     only after step 1, so an orphan never runs on a returned token. Tokens
     whose children were reaped in step 1 went back through their normal
     `Drop`. The jobserver helper thread reads no more bytes from then on;
     a read it had in flight puts its byte straight back, and the cleanup
     waits up to 200 ms for that before going on.
  3. Later token drops return nothing, and new acquisitions are free. Each
     token is therefore returned exactly once.
  4. stderr says `xtask: signal N: killed K child process(es)` and `xtask:
     signal N: returned K jobserver token(s)` (each only when K > 0). These
     lines are written after steps 1 and 2 and ignore write errors, so
     neither a stderr pipe whose reader is gone (EPIPE) nor a full one can
     stop the cleanup.
  5. The signal's default action ends the process, so the parent sees it
     killed by that signal (if that cannot be emulated, `_exit` with status
     128 + signal, without exit handlers).

  **Always exits (WP-1436).** Steps 1 to 4 run on a cleanup thread of their
  own; the signal thread only watches it, and nothing on the signal thread
  can panic. The process ends in every case:
  - when the cleanup is done, or when it panicked;
  - **15 s after the signal at the latest**, whatever the cleanup is stuck
    on: every live child group then gets `SIGKILL`, the held tokens are
    returned, and stderr says `xtask: signal N: cleanup still running after
    15.0s; exiting anyway`;
  - **at once on a second SIGINT or SIGTERM** that arrives 0.5 s or more
    after the first (one within 0.5 s is the same interruption delivered
    twice: `timeout(1)` signals its child and then its process group):
    every live child group gets `SIGKILL`, the held tokens are returned,
    stderr says `xtask: signal N: exiting without waiting for cleanup`, and
    the second signal's default action ends the process.

  No other thread can end the process first or wait forever: a cargo step
  that ended because of the signal, `main` once its command returns, and a
  thread that panics during the cleanup (typically on the dead stderr pipe)
  all wait for the signal thread, and end the process themselves with the
  signal's default action should it not have done so 20 s after they
  started waiting. So an interrupted run never exits with its command's own
  status (or 101 for a panic) before its children are reaped and its tokens
  returned.

  The hang this fixes: the message of step 4 used `eprintln!`, which
  panics on EPIPE. With xtask's stderr a pipe whose reader had died (the
  caller's shell or `| tail` killed together with xtask), the panic ended
  the signal thread before step 5, and the thread that had run the
  interrupted cargo step waited for it forever: one thread, asleep in
  `thread::park`, SIGTERM still caught, a second SIGTERM swallowed by the
  dead thread's handler.

  The memory cap and the timeout are unchanged: they kill one child's group
  and leave the table to its normal reap. Not covered are the helper `git`
  calls, which xtask starts outside `util::process`. They share xtask's
  process group, so a terminal ^C reaches them directly, but a
  `kill -INT <xtask pid>` does not.
- **SIGKILL cannot be handled.** No process can catch it, so `kill -9`
  (including the kernel OOM killer), or any other fatal signal xtask does
  not catch, loses the tokens xtask held, as it would for make and cargo.
  Restore the pool with `cargo xtask jobserver refill`.
- **`cargo xtask jobserver status|refill [--fifo <path>] [--jobs <n>]`.**
  - The fifo is `MAKEFLAGS`' `fifo:` path, else `$KILN_JOBSERVER_FIFO`,
    else `/run/kiln/jobserver.fifo`.
  - `<n>` is `$KILN_JOBS`, else 20, as for `scripts/jobserver.sh`.
  - Both open the fifo read+write with `O_NONBLOCK`. They count the idle
    tokens by reading until `EWOULDBLOCK` and write the same bytes back.
  - `refill` also writes `+` bytes up to `<n>` idle tokens. It is only
    correct while no client holds tokens (nothing building or testing):
    tokens in use come back on top of the refill. It leaves a pool above
    `<n>` alone and reports it.
  - Neither ever blocks. The non-blocking open uses a hard-coded
    `O_NONBLOCK` per OS (Linux, Android, macOS and the BSDs); on other
    platforms both fail with an error.
- **Fallback.** With no `MAKEFLAGS`, the descriptor forms
  (`--jobserver-auth=R,W`, `--jobserver-fds=R,W`, which need inherited
  descriptors), a path that is not a fifo or cannot be opened (a warning
  is printed), or a later fifo I/O error (warning; acquisition becomes
  free), `-j` alone bounds the parallelism exactly as without a jobserver.
- The run header names the pool: `N jobs (jobserver fifo:<path>)`
  (`xtask libcxx`: `(-j N, jobserver fifo:<path>)`).

## Outcomes

The outcomes are evaluated in this order:

1. The file cannot be read, or its directives are invalid: **ERROR**.
2. A `skip` marker applies: **SKIP**.
   So does an unmet `requires: target-<arch>`, `requires: clang>=X.Y.Z`,
   `min-oracle: N`, `requires: clang<X.Y.Z`, `requires: llvm>=N` or
   `requires: llvm<N` (in that order).
3. The test is kiln-specific and the compiler is not kiln: **SKIP**.
4. The test runs. A process that runs out of memory under the cap is
   **KILLED** ("Resource limits"), and a broken test (missing `stdout-file`
   or `run-stdin-file`, oracle rejection, oracle hang or crash (a clang
   oracle's after its one retry, "Oracle"), I/O error)
   is **ERROR**, both even when an `xfail` marker applies.
5. Otherwise, a failure is **FAIL**, or **XFAIL** if an `xfail` marker
   applies. Success is **PASS**, or **XPASS** if an `xfail` marker applies.
   An XPASS counts as a failure, so the stale marker gets removed.

Step 3 also covers `ast`, `layout`, `vtables` and `ir` tests under an
`other` compiler, and `layout`, `vtables` and `ir` tests without an oracle: they are a
**SKIP** with the reason.

## Output

Output goes to stdout:

- One header line: `running N tests with <compiler> (<kind>), oracle: <oracle|none>, [gcc: <path> (<--version line>), ]J jobs, mem limit: <N MiB|none>`; the `gcc:` part appears when a GCC oracle is configured ("GCC oracle").
- One block per result, in name order. A result line is
  `<STATUS> <name> [(<reason>)]`. With `-v`, the line ends with the elapsed time `[<secs>s]`.
- PASS results are printed only with `-v`, unless they carry an oracle
  advisory ("Oracle", "GCC oracle"). SKIP, XFAIL and the failure statuses are always
  printed.
- A failure result is followed by indented details (unified line diffs with
  `-expected +actual`, or `-oracle +actual`) and the commands that were run.
- Last comes the summary:

```
test result: ok|FAILED. P passed; F failed; X xfail; XP xpass; S skipped; E errors; K killed; [G compared with gcc; ][A advisory; ]T tests in S.SSs
```

`G compared with gcc; ` appears only when G > 0: the number of tests GCC
compiled to a verdict (accepted or rejected, "GCC oracle"); a comparison
dropped by an advisory is not counted. `A advisory; ` appears only when a
result carries an oracle advisory: a GCC oracle's dropped comparison, or
a clang oracle's crash that was retried successfully ("Oracle"). A result
with both has one entry, and both notes joined by `; `.

The parity report ("Parity report") counts KILLED in its fail column.

Scratch files live in `<target>/xtask-test/run-<pid>-<nanos>/t<index>/`. Each
test's directory is removed when the test finishes, and the run directory is
removed at the end of the run.

## Expression extents

`cargo xtask extents [options] [paths...]` (WP-2327) checks the written
extents of spec/ast.md §21 against clang: every expression kiln dumps
with `-fkiln-dump-expr-extents` (§21.4) is matched with the node clang's
`-Xclang -ast-dump=json` gives the same file, kind and first byte, and a
written extent kiln states must be the range clang gives that node.

- **Units.** Stage 1 (the default, and run 7 of `ci`): the test files
  under `paths` (default `tests/conformance`) that both front ends can
  take alone with their own `-std=` and `args:`: not `compile-fail`,
  `pp` or `ice`; no `module:`, `unit:`, `header-unit:`, `pch:` or
  `as-header-unit`; not `gcc` (GCC is its oracle: what clang accepts
  may differ); no `skip` for kiln or clang; no `requires-*` or
  `min-oracle`; no `-fkiln*`, `-fclang-compat` or `%{...}` argument; and
  no `#include`, `#import` or `#embed` of a `<...>` header (wall time,
  and the comparison is of the test, not the library). Both run from the
  repository root with `-fsyntax-only`. A unit either front end rejects
  (exit 1) is listed as `rejected <unit>: <who>: <first error>` and not
  compared, as is one clang crashes on (clang's failure, not kiln's); a
  kiln crash, a hang of either (600 s), or a dump that cannot be read is
  `failed <unit>: ...`, a failure. Stage 2 is `--compdb
  <compile_commands.json>` (with `--filter <substr>` on the file names):
  each entry's command without the compiler, `-c`, `-o` and the source;
  it is a report, not gated by the baseline.
- **Clang's side** is read as a stream (`xtask/src/extents/json.rs`, a
  pull reader; `clang.rs`), so a dump of any size takes constant memory.
  Clang prints a location's `file` only when it differs from the
  location printed last in document order, so the reader carries the
  file forward over every location of the dump, read or skipped. Skipped:
  implicit declarations, the specializations a template prints under
  itself (its first templated declaration is the pattern), and
  class and variable template specializations listed under the template.
  The nodes read are the kinds ending in `Expr`, `Operator` or
  `Literal`, and the clang kinds of the kinds table. A node's range is
  `begin.offset` to `end.offset + end.tokLen`.
- **The kinds table** `tests/extents/kinds.txt`: `map <KilnKind>
  <ClangKind>[,...]` (clang's kinds compared as kiln's; `*` is any kind,
  for the implicit wrappers, which have the extent of the written node
  inside them where clang may build another node or none), `none
  <KilnKind> <reason>` (a kind kiln never gives an extent: an extent of
  one is a mismatch), `opaque <ClangKind> <reason>` (clang's dump leaves
  out operands of the kind: a kiln node strictly inside one is compared
  only where clang has a node at the same first byte), `quirk
  <file>:<begin> <reason>` (clang's range there is wrong: kiln's nodes
  beginning there are not compared) and `skip <KilnKind> <reason>` (not
  compared), followed by one to three indented lines of a source on
  which the two front ends build different nodes. A malformed table is
  an error.
- **Matching.** Per unit, clang's file-text nodes are keyed by (file,
  first byte); a kiln extent matches a node of its key with the same
  end whose kind is kiln's or matches it by the table, the node of the
  same kind first. Ends are compared as sets: a node one tree has more
  often at one place (a conversion clang builds twice) is a shape
  difference, not an extent one. A clang node touching a macro gets the
  ranges the rule of spec/lex.md "Token ranges" could give it: when both
  ends are in one argument use, the spelling (when the argument is
  itself a macro, clang's spelling is in that macro's definition, not
  the argument's text: a kiln node of a matching kind inside the
  invocation is then macro-skipped); and the expansion: the begin's file
  offset or invocation start, to the end's file end or the invocation's
  end (the name's end and, when `(` follows, the balanced `)`: clang
  does not say which macros are function-like, so both are candidates);
  a builtin macro's result (`__has_builtin(x)`), spelled in clang's
  scratch space and expanded at the invocation's `)`, also begins at the
  invocation's name. A kiln extent left over is matched against those. A clang node
  touching a macro with no candidate (a scratch-buffer spelling, ends in
  two files) is *macro-skipped*. A kiln extent still left over passes
  when it is the extent of a matched kiln node (an implicit node around
  a written one, where clang builds none), when a clang node of a
  matching kind with no valid begin has its end (clang 19's
  construction of a pack-indexed type), and is not compared when no
  clang node of the file overlaps it (text clang did not compile as an
  expression: the other branch of a conditional inclusion, the message
  of `= delete("...")`). Unmatched clang nodes are *clang-only* (kiln's
  `None`, or a node kiln does not build): counted, never a failure.
  Only the files of the repository are compared in a test unit (a node
  of a system header is dropped), and an initializer list's
  `array_filler` is read as its children. **A unit passes** when every
  kiln extent is matched, no line is `!` (an extent that is no file
  range), and the baseline holds.
- **The baseline** `tests/extents/baseline.txt`: per unit, the FNV-1a
  hash of its text and each kind with a `None` share, `unit <path> <hash>
  <Kind>=<none>/<total>...`. A unit whose text is unchanged may not get
  a larger `None` share of any kind (`none-grew <unit> <Kind>: n/t
  (baseline n/t)`, a failure); a new or changed unit is not gated and is
  counted on a `baseline:` line. `--update-baseline` rewrites the file
  from a whole default run (not with `--shard`, `--compdb` or
  `--filter`). Its `arch <arch>` line names the host architecture it
  was recorded on (`std::env::consts::ARCH`): the system headers a unit
  includes expand differently per architecture (`va_list` is a structure
  on one and an array on another, `NULL` may be `__null`), so the `None`
  shares are gated only on that architecture; elsewhere the run prints
  `baseline: recorded on <arch>, not gated on <host>` and mismatches stay
  gated. A file without an `arch` line is gated everywhere.
- **Output**, deterministic: `extents: <units> units, <n> compared, <m>
  mismatches, <s> macro-skipped`; one `kind <Kind>: some <a> none <b>
  clang-only <c> mismatch <d>` per kind, sorted; then, sorted, the
  `failed`, `mismatch`, `none-grew` and `rejected` lines, `mismatch
  <file>:<begin>-<end> <Kind> kiln "<text>" clang <begin>-<end> "<text>"`
  (clang's node of the same key, else of the same end, else `- ""`);
  `-v` adds a `unit <name>: ok|FAIL` line per unit, `--nones` a `none
  <file>:<offset> <Kind> "<text>"` line per `None` (`none - <Kind>`
  without a file location: a part of a macro body), the inventory
  behind spec/ast.md §21.3. Exit 0 when every
  unit passes, 1 otherwise, 2 on a usage or table error.
- Options: `--kiln` (default: build it), `--clang` (default
  `clang++-19`), `--kinds`, `--baseline`, `--shard K/N` (the sorted
  units, as for `test`), `-j`/`--jobs <n>` (units at once; default the
  CPUs), `--nones`.
- Tests: `xtask/src/extents/tests.rs` (the JSON reader, the file carry
  and the template skip, the table, the baseline, matching with macros
  and sets, what clang cannot show, a builtin macro's invocation begin,
  the unit filter, and spec/ast.md §21.3 against the baseline's kinds).
- Wall time: stage 1 (2808 units, about 198k nodes compared) takes
  about 2-5 minutes at `-j 4` with a debug kiln, under 160 MB RSS.

## The mangling differential

`crates/kiln_codegen/tests/mangle_vs_clang.rs` (WP-980) compiles every
`tests/mangle/*.cpp` with `clang++-19 -S -emit-llvm` for
`aarch64-unknown-linux-gnu` and `x86_64-unknown-linux-gnu`, and checks
that kiln's Itanium mangler (spec/codegen.md) produces each defined
symbol clang emits; it fails below `MIN_SYMBOLS` checked symbols per
triple, so a broken clang run cannot pass it vacuously. It prints one
line per triple, `mangle_vs_clang [<triple>]: <files> files, <n> clang
symbols checked`. Without `clang++-19` it skips with a note, except
under `KILN_REQUIRE_LLVM=1` (GitHub CI), where that is a failure; the
same holds for `abi_run_vs_clang` (WP-1034).

It runs under (WP-1034, closing WP-1031):

- `cargo xtask ci` and `ci --quick`, in the `unit` step (the `unit
  codegen` job in GitHub CI): `cargo test -p kiln_codegen --test
  mangle_vs_clang -- --nocapture`, its own command, so the counts reach
  the log ("CI", step 1);
- `cargo xtask test <paths>` without `--compiler` when a path is, is
  inside, or contains `tests/mangle` (`cargo xtask test tests/mangle`,
  or the default roots' parent `tests`): after the corpus run, the same
  `cargo test` command; its failure makes the exit status 1, and a
  `cargo` that cannot be spawned 2. The corpus run alone only compiles
  the files (`expect: compile`), it does not compare symbols. With
  `--compiler` the differential is not run (it tests kiln's mangler,
  not the compiler under test). The decision is `names_mangle_corpus`.

## The ODR oracle

`scripts/mason/odr_oracle.py` (WP-1581; Python 3 standard library only;
`python3 -I scripts/mason/odr_oracle.py --help`) is the must-equal
oracle of the `.mason.meta` definition rows (kind 2, spec/codegen.md §5.5
"Kind 2"). It takes objects, `ar` archives and directories (searched for
`*.o` and `*.a`), reads every unit of every `.mason.meta` section, and
groups the kind 2 rows of all units by key. It reports (a) keys whose
rows disagree in `hash` among rows of one `odr.scheme` (rows of different
schemes are not compared), (b) K13 pairs: a key with an `EXPLICIT_SPEC`
row and an `IMPLICIT_INST` row, (c) the row count of each flag and
the share of `PARTIAL` rows, and (d) how many keys of (a) disagree
among their rows without `PARTIAL` alone (WP-1583: a complete hash that
differs is a real difference or a hasher bug, never a skipped
construct). Its exit status is 1 when (a) or (b) is not
empty, 2 on a malformed section, else 0. `--names` names each reported
key by the symbol of an object carrying it (FNV-1a 64 of the name);
`--quiet` prints only the counts. It is not part of `ci`: each K-3
package runs it over objects built with `-fmason-metadata` (OSS lanes
built with `KILN_OSS_CXX` set to a wrapper that adds the flag), and
expects zero of (a) and (b). `crates/kiln_driver/tests/mason_defs.rs`
runs it on a K13 pair when `python3` is on `PATH`.

### Context independence of kind 2 rows

No byte of a kind 2 row depends on anything else in its unit (spec/abi.md
"Definition hash"; WP-1586). `tests/mason/context/defs.h` holds one
definition of each row kind and each area of the hash (functions, member
functions, constructors, virtuals, templates and their instantiations,
variables, closures, contracts, attributes, statements, expressions);
`tests/mason/context/preambles/*.h` are the contexts. A preamble starts
with directive comment lines:

- `// sides: pre post|pre|post|wrap`: placed before the unit's text, after
  it (the trailer side), or around it (a `wrap` file splits at a line
  `//@ include`); default `pre post`.
- `// scope: any|defs`: `defs` names `defs.h`'s entities (a
  redeclaration, an explicit instantiation), so it applies to `defs.h`
  only; `any` applies to every unit; default `any`.
- `// expect: differ SYMBOL...`: the redeclaration may change those rows
  (a default argument or `[[noreturn]]` added: a must-differ case); they
  must differ and no other row may.

`crates/kiln_driver/tests/mason_context.rs` compiles `defs.h` plain and
with each preamble on each side at `-std=c++20` and `-std=c++26` (and the
standard-header preamble with libc++ when it is installed), and requires
equal hash and flags for every shared key; a failure names the preamble,
the side and the symbol.
`scripts/mason/odr_preamble.py --kiln PATH [--std S]... [--stdlib L]...
[--arg A]... [--all-scopes] UNIT...` does the same for arbitrary units
(the `scope: any` preambles; `--all-scopes` adds the others) and prints a
`MISMATCH` line per row (unit, standard, library, preamble, side,
symbol, both (hash, flags)); exit 1 on a mismatch, 2 when a plain unit
does not compile. `tests/mason/` is not a default root of the
conformance runner, so these files are not run as tests on their own.

### The O-13 corpus

mortar's O-13 lists the must-equal and must-differ cases of the definition
hash (WP-1586). Each is one directory of `tests/mason/corpus/`, named by
its O-13 id (`N1`, `K7`) or by a short name for the cases without one
(`literal-base`), holding its units, any headers, and `case.tsv`: one
tab-separated line

```
expect  symbols  kinds  level  reason  unit...
```

| column | value |
|---|---|
| `expect` | `equal` (must not be reported), `differ` (must be reported), `excluded` (a documented miss or exclusion, spec/abi.md "Misses and exclusions": not reported) |
| `symbols` | the symbols compared, space-separated; `type:NAME` is the kind 3 row of the type whose Itanium `<type>` is NAME |
| `kinds` | the row kinds that carry the expected difference or equality, comma-separated (`2`; `2,3`; `3`) |
| `level` | the lowest `-fmason-metadata` level writing those rows (`min` for kind 2, `default` with kind 3) |
| `reason` | a stable id for an `excluded` case (`X15`, `X17`, `typedef-noreturn`, `synthesized-noexcept`); `-` otherwise |
| `unit` | `FILE STD [FLAG...]`, space-separated: the source, its `-std` value, its other flags; `{dir}` in a flag is the case's absolute directory; `@pch:H` builds a PCH of `H` with the unit's flags and adds `-include-pch` |

kiln++ runs in the case's directory. `differ` holds when the hashes
differ, or the rows form an `EXPLICIT_SPEC` + `IMPLICIT_INST` pair (K13),
or a row has `USES_INTERNAL` (K12); `equal` needs one hash, no such pair,
and flags equal apart from `VAGUE`, `IMPLICIT_INST` and `EXPLICIT_INST`
(N22, N9); `excluded` needs one hash (the miss), or no row at all for
`X15`. `crates/kiln_driver/tests/mason_corpus.rs` checks every case.
`python3 -I scripts/mason/odr_corpus.py --kiln PATH --out DIR [--level
L] [--check] [CASE...]` builds every case's objects into `DIR/CASE/`
(with `-fmason-metadata=L`, default `default`) and writes
`DIR/manifest.tsv` for a linker's test lane, one line per case after a
`#` header:

```
case  expect  symbols  kinds  level  reason  (object  std  flags)...
```

with one (`object` relative to DIR, `std`, `flags` space-separated with
`{dir}` expanded, `-` when none) triple per unit, in the case's order;
`--check` also verifies the expectations as above (exit 1 on a failure).

## CI

The libc++ suite driver (`cargo xtask libcxx`) is not part of `ci`. It is a
separate manual or periodic gate, and its contract is `spec/libcxx.md`.
Neither is the libstdc++ header status (`cargo xtask libstdcxx`), which is
informational (exit status non-zero only on tool errors); its contract is
`spec/libstdcxx.md`. Rules that act only before C++17 (the second step of
a C++14 copy-initialization, pre-C++17 copy elision) are gated by its two
lowest levels, run by hand before landing such a rule:
`cargo xtask libstdcxx --std c++14 --clang clang++-22 -j 4` and the same
with `--std c++11` (about 10 s each once kiln is built, WP-1513). At both
levels every header but `<coroutine>` (which needs `-fcoroutines` before
C++20, and the reference rejects too) is accepted. The libc++ gate has no
`-std` option: its feature set (`xtask/src/libcxx/features.rs`: the
`c++26` feature and the `std-at-least-*` list), the `-std=` flag of
`run.rs` and the report's flags are fixed at C++26, and the M3 gate's
recorded results are C++26 results.

The M8 OSS build harness (`scripts/oss/`) needs the network and is not part
of `ci`; its contract is spec/landed/harness/WP-666.md. It builds each
project against libstdc++ by default, and against the system libc++ 19
(the library clang++-19 uses, so kiln and the clang-19 control link the
same one) with `oss-build.sh --stdlib libc++`, whose build trees and
reports are kept apart (`<compiler>-libcxx`, WP-694). Rows that use
C++20 modules (`modules = true`) build with Ninja and the compiler's
dependency scanner, kiln-scan-deps for kiln and clang-scan-deps-19 for
the control (spec/landed/harness/WP-1148.md).

`cargo xtask features` ("Feature table") is not part of `ci` either: it is a
manual / M4-gate check.

`cargo xtask ci [--quick] [--oracle-only] [--step <steps>] [--shard K/N]
[--unit-group <group>]` is the merge gate (`docs/AGENTS.md` rule 3). It
runs, stopping at the first failing step:

0. four in-process lints, each listing every problem it finds:
   `cargo xtask diag-split --check` (kiln_diag's id and group tables are
   in their area files with empty tails, spec/diag.md §2.4; the failure
   says to run `cargo xtask diag-split`), and the per-WP Landed files
   (spec/README.md "Landed sections", `xtask/src/landed.rs`): each file
   under `spec/landed/` is `<spec>/WP-NNN.md` for an existing
   `spec/<spec>.md`, starts `# WP-NNN — <title>` with its own id, and has
   a `| WP-NNN |` row in `docs/BACKLOG.md`; the local-path lint
   (`xtask/src/local_paths.rs`, WP-1886): no file `git ls-files` lists,
   other than binary (non-UTF-8 or NUL-holding) files, holds an absolute
   path starting `/Volumes/`, `/Users/`, `/home/<user>/` (a lower-case
   name and a slash), `/root/` or `/tmp/claude`, followed by more of the
   path (a bare prefix in prose is not a path), where the `/` starts a
   path (line start, after a character that cannot be part of a path, or
   after a one-letter flag such as `-I`); `/home/linuxbrew/` (Homebrew's
   fixed prefix) is allowed (`local_paths::ALLOWED`), and so is the lint's
   own source (`local_paths::ALLOWED_FILES`); each hit is reported as
   `file:line`; and the static-gap inventory
   (`cargo xtask static-gaps`, `xtask/src/static_gaps.rs`, WP-1719). A
   *gap site* is a line of a crate's `src/` outside test modules that
   uses a gap diagnostic (`static_gaps::GAP_DIAG_IDS`), calls a function
   that forwards its text to one (`static_gaps::WRAPPERS`, by crate), has
   a string literal with a gap phrase (`static_gaps::PHRASES`: "not
   supported yet", "not yet implemented", ...), or uses `todo!` /
   `unimplemented!`. Each site carries `// kiln-gap: GAP-NNN` on its line
   or in the comment block directly above it (`forward` inside a wrapper
   function, `check` on a line that only compares an id with a gap id),
   and a `GAP-NNN` marker no detected site uses marks the next code line
   as a site. A paragraph of a top-level `spec/*.md` whose bold lead is a
   gap phrase (`static_gaps::SPEC_LEADS`: `**Not implemented`,
   `**Unsupported`, ...) is a spec site, for a gap with no text of its
   own in the code (WP-1782); it carries `<!-- kiln-gap: GAP-NNN -->` on
   its line or the line above and is listed as `spec/<file>.md::§<heading>`.
   A bold lead that opens a list of gaps or limits
   (`static_gaps::SPEC_LIST_LEADS`: `**Gaps`, `**Known gaps`,
   `**Limitations`, `**Missing`, `**Open`, `**Remaining`, `**Deferred`,
   ..., as the whole bold text or followed by `:`, `.` or ` (`; WP-1797)
   makes each unit under it a spec site: the lead's own paragraph when
   text follows it, and each list item below it. Such a unit carries a
   `GAP-NNN` marker or names an open `docs/BACKLOG.md` row (`WP-NNN`).
   The lint fails when a site has no marker or names an id
   `docs/status/static-gaps.md` lacks, when an entry there has no site
   left, when an entry's sites cell (each `` `file::fn` ``, sorted, ` ×N`
   for N sites) differs from its markers, or when its row is not an
   `open` / `in progress` row of `docs/BACKLOG.md` or `excluded:
   <reason>`, or when a unit under a gap-list lead has neither a marker
   nor an open row; line numbers play no part. All four run before `ci` fails;
1. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
   warnings`, `cargo test --workspace`. A step that fails with rustc's
   `error[E0463]` (can't find crate) on a line of its stderr is rerun
   **once**, after `xtask ci: [retry] cargo <step> failed with error[E0463]
   ...`: a dependency artifact vanished under the build (as when another
   cargo process shares the target directory), and a rerun rebuilds it. Any
   other failure, or a second E0463, fails `ci`. `ci` copies each step's
   stderr through as it arrives. The decision is
   `test::report::ci_should_retry`. `cargo test --workspace` (and the
   `codegen` unit group) skips `mangling_matches_clang`, which then runs
   as a command of its own, `cargo test -p kiln_codegen --test
   mangle_vs_clang -- --nocapture` ("The mangling differential", WP-1034);
2. `cargo build -p kiln_driver` (the binary the kiln-mode run tests, in the
   target dir as for `--compiler`'s default);

then the corpus runs, all over the default paths (`tests/c`, `tests/conformance`,
`tests/debuginfo`, `tests/differential` and `tests/stress`) with the default oracle
`clang++-N` (N from `KILN_ORACLE`, default 22; "Oracle toolchain"), the
default GCC oracle ("GCC oracle") and the
default `--mem-limit` (8192 MiB). A bad `KILN_ORACLE` fails `ci` (exit 2)
before step 0, and so does `KILN_REQUIRE_GCC=1` without a usable GCC. All run even
if the first fails, so one cannot hide the other:

3. **clang self-check**: `cargo xtask test --compiler clang++-N` (printed
   as `corpus (clang++-N self-check)`). The corpus
   must pass with the reference compiler, or the corpus is wrong. It is
   skipped, with the reason, under `--quick` or when `clang++-N` is not on
   PATH (`--version` cannot be run).
4. **kiln mode**: `cargo xtask test --compiler <the kiln built in step 2>`.
   It is never skipped. It needs clang N (the oracle, and kiln's linker), so
   without clang it ends in an infrastructure error and `ci` fails.
5. **module-file kinds** (WP-985): `cargo xtask test --compiler <that
   kiln> --bmi-kind 1`, then `--bmi-kind 2` (printed as `corpus (kiln
   --bmi-kind N)`; "Module-file kinds"), then `--bmi-kind 2 --bmi-image`
   (WP-1828, printed as `corpus (kiln --bmi-kind 2 --bmi-image)`: the
   imports the declaration image covers). They run with run 4, are
   selected by `--step kiln`, take the same `--shard`, and are judged like
   it.
6. **PCH differential** (WP-1036): `cargo xtask test --compiler <that
   kiln> --pch-diff replay`, then `--pch-diff
   snapshot --pch-diff-require-restore` (WP-1395: a consumer that stops
   restoring its snapshot fails CI), over
   `tests/conformance/cpp.include` and `tests/conformance/module.interface`
   only (printed as `corpus (kiln --pch-diff <how>)`; "PCH differential").
   Like run 5 they are selected by `--step kiln`, take the same `--shard`
   and are judged like run 4; `--oracle-only` skips them (they run no
   oracle).
7. **Expression extents** (WP-2327): `cargo xtask extents --kiln <that
   kiln>`, stage 1 ("Expression extents"), printed as `corpus (kiln
   extents)`; its line counts units (a unit with a mismatch, a failure or
   a grown `None` share is a FAIL; a test file that is no unit is a
   SKIP). Selected by `--step kiln`, takes the same `--shard`;
   `--oracle-only` skips it.

`ci` then prints one line per corpus run, `xtask ci: corpus (<run>): <line>`,
where `<line>` is the run's summary line ("Output"), `skipped (<reason>)`, or
`error: <message>`. **`ci` passes only if** every step of 1–2 succeeds and
no corpus run has a FAIL, XPASS, ERROR or KILLED (XFAIL and SKIP are allowed)
or an infrastructure error; otherwise it exits 1. The decision is
`test::report::ci_corpus_passes`. A kiln gap is therefore recorded as
`xfail(kiln):` (or `skip(kiln):`) with the reason, never left failing.

`--quick` is for local iteration only; merges to `main` and GitHub CI use the
full form. Any other argument is a usage error (exit 2).

**`--step`, `--shard`, `--unit-group`** (WP-692) cut `ci` into the slices
GitHub CI runs as parallel jobs; together the slices are exactly the full
form. `--step <steps>` (a comma list, or the flag repeated; default every
step) selects among `lints` (step 0 and fmt and clippy), `unit` (`cargo
test --workspace`, or under `--oracle-only` the `ORACLE_TESTS` targets),
`self-check` (run 3; step 2's build is not needed for it) and `kiln` (step
2 and run 4); an unknown name is a usage error. The bad-`KILN_ORACLE` and
`KILN_REQUIRE_GCC` checks run in every invocation. An unselected step
prints `xtask ci: <step>: skipped (--step)`, and an unselected corpus run
reports `corpus (<run>): skipped (--step)`, which passes like a `--quick`
skip; `ci_corpus_passes` judges the runs that ran. `--shard K/N` is passed
to both corpus runs as `cargo xtask test --shard K/N` ("Command line": the
sorted default list, test `i` in shard `K` iff `i % N == K - 1`), and the
corpus lines then name the shard (`corpus (kiln, shard 2/6): ...`); with
neither `self-check` nor `kiln` selected it is a usage error.
`--unit-group <group>` runs one group of the `unit` step: `cargo test -p
<pkg>...` for a group of `UNIT_GROUPS` (`xtask/src/main.rs`: today `xtask`
and `codegen` = `kiln_codegen`), or `rest` = `cargo test --workspace
--exclude <every grouped package>`, so a new crate falls into `rest` and
the groups always cover the workspace. It needs `unit` selected and not
`--oracle-only` (usage error otherwise). `ci_tests` checks the groups are
disjoint packages that exist, and that `.github/workflows/ci-lane.yml`'s
unit matrix lists every group. Each selected step prints its time
(`xtask ci: unit: done in 61.2s`, `xtask ci: kiln built in 48.0s`), and
so does each cargo command of a step when it ends (WP-1436): `xtask ci:
cargo <args>: ok in 12.3s`, or on failure `xtask ci: failed with <exit
status> in 12.3s`. Each corpus run's line ends with its own time (`... N
tests in 123.45s`). The last line of every `ci` run that is not
interrupted, passed or failed, is the total: `xtask ci: total: 1234.5s`.

**`--oracle-only`** (WP-488) is the second oracle's gate: GitHub CI runs
`KILN_ORACLE=19 cargo xtask ci --quick --oracle-only` after `cargo xtask
ci`. It skips step 0, fmt and clippy, and replaces `cargo test
--workspace` by `cargo test -p <pkg> --test <name>` for each test target
in `ORACLE_TESTS` (`xtask/src/main.rs`), the targets whose result depends
on `KILN_ORACLE` (today `kiln_codegen`'s `ir_vs_clang`); steps 2–4 are as
above. Everything else in the workspace either pins clang 19 itself or
uses no oracle, so it already ran, with the same result, in the
default-oracle pass: the crates' tests name `clang++-19` and
`/usr/lib/llvm-19` (kiln's backend through `--llvm-dir=`), and the harness's own end-to-end tests
(`xtask/src/test/tests.rs` `clang_opts`) pin release 19 as compiler,
oracle and toolchain (`Options::toolchain`, so `requires: llvm` does not
follow `KILN_ORACLE` there either). A test target that reads `KILN_ORACLE` must be
listed: `ci_tests::oracle_tests_are_listed` scans `crates/` for the name
and fails on an unlisted (or stale) target. `--oracle-only` with the
default oracle is allowed, but checks nothing the full form does not.

The end-to-end tests in `xtask/src/test/tests.rs` and
`xtask/src/bench/tests.rs` skip themselves with a note when clang 19 is
missing. GitHub CI (`.github/workflows/ci.yml`) runs the equivalent of `cargo
xtask ci` and then `KILN_ORACLE=19 cargo xtask ci --quick --oracle-only`,
fanned out over parallel jobs (WP-692; the job graph is in
docs/VALIDATION.md "GitHub Actions"), each in a
`gcc:16.1.0-trixie` container, on `ubuntu-24.04-arm`, the reference
environment (aarch64 Debian 13, clang 19.1.7, GCC 14 for libstdc++
detection, plus GCC 16.1 in `/usr/local` as the GCC oracle), with
`clang-19 llvm-19 lld-19 libc++-19-dev libc++abi-19-dev` installed,
`KILN_REQUIRE_LLVM=1`, `KILN_GCC=/usr/local/bin/g++`,
`KILN_REQUIRE_GCC=1`, `KILN_REQUIRE_DEBUGGERS=1` and
`KILN_REQUIRE_SANITIZER_RUNTIME=1`, so no capability skip happens there
(docs/VALIDATION.md "GitHub Actions", WP-226, WP-502, WP-816, WP-1034):
a test whose capability is missing is an ERROR, not a SKIP. The corpus
jobs install compiler-rt for both clangs (`libclang-rt-19-dev
libclang-rt-22-dev`, the setup action's `sanitizer-runtimes` input), so
the `requires: sanitizer-runtime` tests run on both architectures
(`ci_tests::workflow_installs_the_sanitizer_runtimes`). Shard 1 of
`kiln-22` also installs `cmake ninja-build` (the `cmake` input) and,
after its corpus step, runs `scripts/oss/selftest.sh --cmake-modules`
with the `target/debug/kiln++` it built: the CMake modules end-to-end
(WP-982), which there fails rather than skips without CMake 3.28 and
ninja (`ci_tests::workflow_runs_the_cmake_modules_e2e`). The corpus jobs (`self-check`, `kiln-22`, `kiln-19`) and the
`unit xtask` job (the debugger fixtures) also install `gdb lldb-19`
(the setup action's `debuggers` input) in a container started with
`--cap-add=SYS_PTRACE`; `ci_tests::workflow_installs_the_debuggers`
checks that every corpus job does. The action first checks that
`$KILN_GCC -dumpfullversion` is 16.1.x and that it compiles a `<meta>`
TU with `-std=c++26 -freflection`.
The fan-in jobs `check` (aarch64) and `check-x86_64` (the same container
on the x86-64 `ubuntu-24.04` runner, WP-228), the required status checks,
pass only when every job of their architecture succeeded; the jobs share
their setup through the composite action `.github/actions/kiln-ci`
(WP-239, WP-692).

### Misbehaviour checks

Three invariants a compiler and its harness hold anyway, enforced on
GitHub CI by the `guard` job of `.github/workflows/ci-lane.yml` (aarch64
lane only; WP-1901) through `cargo xtask guard` (`xtask/src/guard.rs`).
They are not steps of `cargo xtask ci`: strace is not part of the
development setup, and the job runs in parallel with the others.

**No network, no stray writes.** `cargo xtask guard trace [--log <file>]
[--allow <path>]... -- <command>` runs the command under `strace -f -y
-qq -o <log> -e trace=<guard::TRACED>` (default log
`<target>/guard/strace.log`) with `TMPDIR` set to a fresh
`<target>/guard/tmp-<pid>` and `CARGO_TARGET_DIR` to the target
directory, then checks the log (`guard check-log <file> [--allow
<path>]...` checks an existing one). It fails (exit 1) when the command
fails, when the log holds no call, or when:

- any traced process calls `socket` with a family outside
  `guard::ALLOWED_FAMILIES` (empty: on a clean run no process of the
  slice opens a socket of any family, so `AF_INET`, `AF_INET6`,
  `AF_UNIX` and `AF_NETLINK` all fail), whether or not the call
  succeeded, or `connect`s or `bind`s an `AF_INET`/`AF_INET6` address;
- a **successful** call writes a path outside the allowlist. Writes are
  `open`/`openat`/`openat2` with `O_WRONLY`, `O_RDWR`, `O_CREAT`,
  `O_TRUNC`, `O_APPEND` or `O_TMPFILE`, `creat`, `truncate`,
  `mkdir[at]`, `mknod[at]`, `unlink[at]`, `rmdir`, both paths of
  `rename[at|at2]`, and the new name of `link[at]` and `symlink[at]`. A
  path is the argument as written, made absolute with the directory
  `-y` prints for its dirfd (`AT_FDCWD</cwd>`, `5</dir>`) and `.`/`..`
  removed lexically; a relative path with no directory (the plain
  x86-64 forms) cannot be judged and fails. The allowlist (each entry
  allows itself and everything below it, also in its canonical form):
  the target directory (test scratch directories, `xdg-cache`, the
  run's `TMPDIR`), `/dev/null`, `/proc/self`, `/proc/thread-self`, the
  jobserver FIFO a `--jobserver-auth=fifo:<path>` in `MAKEFLAGS` or
  `CARGO_MAKEFLAGS` names (opened read-write for tokens, written
  nothing to; a development machine's pool), and each `--allow`
  (relative to the root). Read-only opens and failed writes pass.

The job builds first (`cargo build -p kiln_driver -p xtask`), so cargo's
writes to `$CARGO_HOME` are not traced, then traces
`target/debug/xtask test --compiler target/debug/kiln++` over
`tests/conformance/basic.start.main` (run tests),
`tests/conformance/basic.link` and `tests/conformance/class.static.data`
(linking, a link failure) and `tests/conformance/module.unit` (module
units, two-step builds): 108 tests, about 15 s locally. The strace runs
in the job's ptrace-enabled container. The log line is `xtask guard:
ok: <calls> calls of <processes> processes, 0 network and 0 write
violations`, and each violation is listed (`xtask guard: network:
AF_INET: <call>`, `xtask guard: write: <path>: <call>`, the first 40 of
each).

**Risky-change flags.** On a `pull_request` run the job first runs
`cargo xtask guard risky --base HEAD^1 --head HEAD` (the merge commit
and its base; checkout depth 2). It diffs the two (`git diff
--name-status --no-renames`) and flags:

- a new or changed entry of a dependency table of any `Cargo.toml`
  (`[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, under
  `workspace.` or `target.<cfg>.`, a `[dependencies.<name>]` table,
  `[patch.<source>]`, `[replace]`), and a `[package] build =` key;
- a new or changed external package of any `Cargo.lock` (one with a
  `source`; the workspace's own crates' version bumps are not flagged);
- a new `build.rs`;
- a manifest that becomes `[lib] proc-macro = true`;
- any change under `.github/` or `.cargo/` (linker, runner and rustflags
  settings run code at build time too).

Each flag is a `::warning file=<path>,title=Risky change: <kind>::<detail>`
annotation, and the list goes to the job summary (`$GITHUB_STEP_SUMMARY`;
printed when it is unset). The step **does not fail** (exit 0; `--strict`
makes a flag exit 1, for a later gate): kiln's own pull requests change
dependencies and workflows on purpose (this one changes `.github/`), and
a failing check would need an override that every such PR takes, which
trains reviewers to click past it. The annotation shows on the PR's
checks and files, where the reviewer sees it before merging; screening
outside contributions before they reach CI is a separate gate. A removed
dependency or package, a deleted `build.rs` and an edit of an existing
one are not flagged (the edit is ordinary review).

**Read-only default token.** Every workflow sets `permissions: contents:
read` at top level (`ci.yml`, `ci-lane.yml`, `sweep.yml`,
`release.yml`). No job of `ci.yml`/`ci-lane.yml` writes to the
repository or calls the API: caches (`Swatinem/rust-cache`) and
artifacts use the runner's own token, and the merge queue needs only
the checks. The one job with more is `release.yml`'s `release`
(`contents: write`, to create the draft release).
`ci_tests::workflows_default_to_a_read_only_token` fails on a workflow
without the block and on a job-level `permissions:` outside
`ci_tests::WRITE_JOBS`; `ci_tests::workflow_runs_the_guard` checks the
job's shape and that its slice directories exist. The parser and both
rules are tested on canned logs in `xtask/src/guard/tests.rs`
(`an_inet_socket_fails` is the deliberate failing case).

**Tier-0** (WP-1902; docs/VALIDATION.md "Tier-0 (outside pull
requests)"). Pull requests from forks are screened by a pipeline outside
this repository (a prefilter, then tier-0, then a static triage, then the
full CI above); GitHub holds each of their workflow runs for approval, and
the pipeline approves only the tier-0 run of a head sha and reads only its
conclusion. The contract it relies on:

- **Path**: the workflow is `.github/workflows/tier0.yml`, matched by that
  file path; the full CI stays in `ci.yml`.
- **Trigger**: `pull_request` only (no `pull_request_target`,
  `workflow_run`, push or schedule).
- **Permissions**: top-level `permissions: contents: read` and none per
  job; no `secrets.` reference, no GitHub API call, no uploaded artifact;
  the cache (`ci-x86_64`) is restored, never saved; the checkout keeps no
  credentials.
- **Jobs**: `tier0 lints` (`cargo xtask ci --step lints`: step 0, fmt,
  clippy) and `tier0 unit` (`cargo test --workspace --lib --exclude
  xtask`: every crate's library unit tests; no integration test, no
  xtask test, no corpus), both on `ubuntu-24.04` (x86_64) in the
  `gcc:16.1.0-trixie` container with the `.github/actions/kiln-ci` setup
  and `KILN_REQUIRE_LLVM=1`, `KILN_GCC=/usr/local/bin/g++`,
  `KILN_REQUIRE_GCC=1`. Each has a `timeout-minutes` (15 and 25).
- **Same-repo skip**: each job has `if:
  github.event.pull_request.head.repo.full_name != github.repository`, so
  a same-repo pull request's run has only skipped jobs.
- **Concurrency**: `tier0-<pull request number>`, `cancel-in-progress:
  true`: a newer push cancels the same pull request's older run, and runs
  of different pull requests never cancel each other. Rate limiting is
  the pipeline's.
- **Conclusion**: `success` means both jobs ran and passed, so fmt,
  clippy, the step-0 lints and the library unit tests passed on x86_64 at
  that head. Any other conclusion (`failure`, `cancelled`, `timed_out`,
  `skipped`, ...) is not a pass. It says nothing about the corpus,
  aarch64, the integration tests or the oracles; those are the full CI's.

These hold for the file on `main`: a pull request runs its own merge
commit's copy of the workflow, the setup action and xtask, so a pull
request that changes `.github/` or `xtask/` can change what its tier-0 run
does, and screening that is the prefilter's. `tier0_tests` in xtask
(`xtask/src/tier0_tests.rs`) checks the trigger, permissions, timeouts,
secrets, same-repo skip and concurrency group on the real file, and that
a mutated copy breaks each one.

Landed (WP-096): the kiln-mode run, `--quick`, and the pass criteria above.
Before it, `ci` ran only the clang self-check, and kiln regressions in the
corpus went through unnoticed.

Landed (WP-125): the address-space cap ("Resource limits"), on by default in
`xtask test`, `xtask bench` and both `ci` corpus runs; the KILLED outcome;
`mem<=N`; `tests/stress/` as a default root (the stress tests moved there
from `tests/conformance/stress/`, with
`tests/stress/dump-friend-template-cycle.cpp`, the AST dumper cycle of
2026-10-31, under `mem<=512`); and the E0463 retry (WP-124).

## Dist

`cargo xtask dist [--target <triple>] [--out <dir>] [--check] [--keep]
[--pgo [--llvm-dir <dir>]]` (`xtask/src/dist.rs`, WP-927; `--pgo` makes
step 1 profile-guided, "PGO build") builds the release archives of the install
layout (spec/driver.md "Installation layout"). It is not part of `ci`: a
`dist` build is a full thin-LTO release build, several minutes. The
tag-only workflow `.github/workflows/release.yml` runs it with `--pgo
--check` on both CI architectures (the published archives are
profile-guided, WP-1862) and attaches the results to a draft GitHub
release; a run of `cargo xtask dist --pgo --check` locally is the
pre-tag check (the release gate's `archive` step, "Release gate"), and
a manual run of the workflow (`gh workflow run release.yml --ref
<branch>`) is the same pipeline without the release, the pre-tag check of
the workflow itself.

1. **Build.** `cargo build --locked --profile dist -p kiln_driver
   --features dist [--target <triple>]`. The `dist` profile
   (`Cargo.toml`) is `release` (thin LTO, line-table debug information)
   unstripped; the `dist` feature compiles the source-tree resource
   fallback out (spec/driver.md "Toolchain"). `CARGO_ENCODED_RUSTFLAGS`
   gets the caller's flags plus `--remap-path-prefix` for the checkout
   (`/kiln`), the target directory (`/kiln/target`), `$CARGO_HOME`
   (`/cargo`) and `$RUSTUP_HOME` (`/rustup`), each as given and
   canonicalized.
2. **Split.** `objcopy --only-keep-debug` the binary to `kiln.debug`, then
   `objcopy --strip-debug --add-gnu-debuglink=kiln.debug` it to
   `bin/kiln` (`$OBJCOPY`, else `objcopy`, else `llvm-objcopy-19`).
3. **Assemble** `kiln-<version>-<triple>/` (`dist::assemble`): `bin/kiln`,
   `bin/kiln++ -> kiln`, `lib/kiln/<major>/include/` (all of
   `resource/include`), `lib/kiln/<major>/include-clang22/` (all of
   `resource/include-clang22`), `lib/kiln/<major>/predefines/*.txt`,
   `share/doc/kiln/` (`dist::DOCS`); directories 0755, files 0644, the
   binary 0755. The docs install flat (WP-1027): `README.md`,
   `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, the two licenses and
   `docs/{COMPATIBILITY,KNOWN_ISSUES,STABILITY,USER_GUIDE}.md` all land in
   `share/doc/kiln/`. Each Markdown file's relative links
   (`[t](target)`, `![a](target)`, HTML `src="..."`/`href="..."`, outside
   code; `xtask/src/doclinks.rs`) are rewritten by
   `dist::rewrite_links`: a link to another shipped doc becomes its file
   name (anchor kept), a link to any other checkout path becomes
   `https://github.com/christopherfretz/kiln/{blob,tree,raw}/v<version>/<path>`
   (`raw` for `src=`, `tree` for a directory), and a link to a path the
   checkout lacks fails the step. Then `doclinks::check_tree` checks the
   installed tree: every relative link names a file in it, and every
   `#anchor` is a heading slug (GitHub's rule: lower case, punctuation
   but `-`/`_` dropped, spaces as `-`, repeats numbered `-1`, `-2`, ...)
   or an HTML `id`/`name` of the target; a broken link fails the step.
4. **Archive** (`dist::tar_xz`), into `--out` (default
   `<target-dir>/kiln-dist`): `kiln-<version>-<triple>.tar.xz` with the
   stripped binary and `kiln-<version>-<triple>-symbols.tar.xz` with the
   unstripped one, then `SHA256SUMS` (`sha256sum` of both), then
   `build-info.txt` (`dist::BuildInfo`, WP-1862), which is not in
   `SHA256SUMS` and not published: `key: value` lines `version`,
   `target`, `commit` (`git rev-parse HEAD`, empty without git) and `pgo`
   (`yes` or `no`); with `yes` also `profile-sha256` (`sha256sum` of the
   merged `.profdata`), `profile-bytes`, `llvm-profdata` (its version),
   `training-cases`, `unprofiled-kiln` and `unprofiled-host-code`
   (the final build's per-function counts, "PGO build"). An older
   `build-info.txt` in `<out>` is removed when the run starts. Archives are
   reproducible: GNU tar `--sort=name --format=gnu --owner=0 --group=0
   --numeric-owner --mtime=@<epoch>`, normalized modes, `xz -9 -T1`; the
   epoch is `$SOURCE_DATE_EPOCH`, else HEAD's commit time, else 0. The
   sizes are printed. `<version>` is the workspace version
   (spec/driver.md "Version").
5. **`--check`** extracts both archives under `<out>/check-<pid>/` and
   runs, in a directory outside the checkout (beside the extraction when
   `<out>` is outside it, else the system temporary directory), with
   every `KILN_*` variable removed from the environment and the
   extracted `bin/` first on `PATH`:
   - the layout (step 3) is there, `bin/kiln++` links to `kiln`, and
     the doc link check (step 3) passes on the extracted tree;
   - `kiln++ --version` names the version;
   - `-print-resource-dir` through `PATH`, `bin/kiln` and a symlink to
     `bin/kiln++` from elsewhere prints `<prefix>/lib/kiln/<major>`;
   - hello world (`<iostream>`) compiles, links and prints its line;
   - a constexpr-heavy TU (a sieve over `new[]` and a `std::vector`
     sort, `-std=c++26 -O1 -fconstexpr-steps=100000000`) compiles and
     runs;
   - a `<meta>` reflection TU compiles at `-std=c++26 -freflection` with
     `--gcc-toolchain=` of the GCC 16.1 oracle ("GCC oracle"; skipped
     without one unless `KILN_REQUIRE_GCC=1`, which makes it an error);
   - no build-host path (step 1's list) appears in either archive's
     binary or in anything the steps printed (the extracted prefix and
     the scratch directory blanked out first);
   - the stripped binary's `.gnu_debuglink` names `kiln.debug` with the
     CRC-32 of `objcopy --only-keep-debug` of the symbols archive's
     binary.

   The directories are removed afterwards unless `--keep`. Tests:
   `xtask/src/dist.rs` `tests` (options, `build-info.txt`, the CRC, the path scan, the
   layout with the installed docs' link check, the link rewriting, and a
   byte-identical re-archive) and `xtask/src/doclinks.rs` `tests` run in
   `cargo test`; the build
   and `--check` run in release.yml and by hand.

## PGO build

A profile-guided build of kiln itself (WP-1308, `xtask/src/pgo.rs`): the
same source and Cargo profile, optimized with a profile of kiln compiling
the training corpus below. It is opt-in; without the switch every command
here builds exactly what it did before. Not part of `ci` (it is three
release builds' worth of time). The published release archives are PGO
builds (`release.yml`, "Dist"), and so is the binary the release gate
tests ("Release gate"); a stock `cargo build --release` stays plain, and
`pgo-profile` stays the packager's way to a PGO build of their own.

**Commands.**

- `cargo xtask build [--pgo] [--target <triple>] [--features <list>]
  [--llvm-dir <dir>]`. Without `--pgo`: exactly `cargo build --release -p
  kiln_driver` (plus `--target`/`--features`), environment untouched. With
  `--pgo`: `pgo-profile` into `<target-dir>/pgo/release/kiln.profdata`,
  then the same release build with `-Cprofile-use=<that path>`, so the
  binary lands where `cargo build --release` puts it.
- `cargo xtask dist --pgo [--llvm-dir <dir>]` ("Dist"): the same with the
  `dist` build's cargo arguments and rustflags (its `--remap-path-prefix`
  set included) for both builds, state under `<target-dir>/pgo/dist/`;
  packaging and `--check` are unchanged.
- `cargo xtask pgo-profile --out <file> [--target <triple>] [--features
  <list>] [--llvm-dir <dir>] [-v]`: the pre-step alone (steps 1-3), for a
  packager whose own build is a stock `cargo build --release`.
- `cargo xtask pgo-train --compiler <kiln++> --profile-dir <dir> [-v]`:
  step 2 alone, for an instrumented binary built some other way; it then
  prints the `llvm-profdata merge` line with the discovered tool.
  `--list` prints the training corpus and the held-out set.

`--pgo` (and `pgo-profile`) with a `--target` other than the host is an
error: the training runs the instrumented binary on the build machine.

**Steps.** Every entry point runs the same functions, so the all-in-one
switch and the step-by-step path cannot drift:

1. *Instrumented build:* the plan's `cargo build` with
   `CARGO_TARGET_DIR=<target-dir>/pgo/<flavour>/instrumented` and
   `-Cprofile-generate=<target-dir>/pgo/<flavour>/profiles` added to the
   rustflags. The profile directory is then emptied (the instrumented
   build scripts write profiles of their own while the build runs).
2. *Training* (`pgo::train`): the instrumented `kiln++` compiles each
   training case once through the bench harness's own command
   construction (`bench::train`, which shares `bench::prepare` with the
   measurement: `-std=`, the mode's flags, `args:`, the reflection shapes'
   `--gcc-toolchain`, the counter-report flags, `generate:` sources, the
   `mode: run` program build and the memory cap), with
   `LLVM_PROFILE_FILE=<dir>/kiln-%m.profraw` (one merged file per binary)
   and `KILN_RESOURCE_DIR=<checkout>/resource` unless already set (a
   `dist` binary has no source-tree fallback). A case that fails fails the
   command, naming every failed case; so does a run that leaves no
   `.profraw` file (the compiler was not instrumented).
3. *Merge:* `llvm-profdata merge -o <out> <dir>/*.profraw` (printed).
4. *Final build:* the same plan in the normal target directory with
   `-Cprofile-use=<absolute path>` and
   `-Cllvm-args=-pgo-warn-missing-function`; xtask hides those per-function
   warnings and prints how many there were, in kiln's crates and in the
   code that runs in the build (build scripts and the workspace's
   proc-macro crates, `pgo::proc_macro_crates`; below).

**Rustflags.** Both builds get the caller's flags as cargo reads them
(`CARGO_ENCODED_RUSTFLAGS`, else `RUSTFLAGS`, else
`CARGO_BUILD_RUSTFLAGS`; a `[build] rustflags` in a cargo config file is
not read) with any `-Cprofile-use` / `-Cprofile-generate` removed, then
pass them as `CARGO_ENCODED_RUSTFLAGS`. The `-Cprofile-use` path is always
absolute: a relative one is resolved per crate and fails.

**What must match between the two builds** (checked on rustc 1.95 with
`-pgo-warn-missing-function`, WP-1308): the rustc, the source tree, the
`--target` argument (an explicit `--target <host>` is a different build
from none: every function misses), the feature set (the crate whose
features differ misses entirely), `-C debuginfo` and
`--remap-path-prefix` (a few internal functions, such as drop glue, miss).
The target directory, the Cargo profile's name (`release` against
`dist`), and whether cargo is run by xtask or directly do not matter.

**Telling that a profile applied.** A missing profile file is a hard
error from rustc. A profile that does not match the build is not: rustc
just optimizes without it. `-Cllvm-args=-pgo-warn-missing-function` makes
rustc warn once per function the profile has no data for (`warning: <cgu>:
no profile data available for function <symbol> ...`). Functions the
training never reached are in the profile with zero counts and do not
warn, so a matching profile leaves no kiln function without data (WP-1308
measured 0 in kiln's crates, with and without a packager's `-C
debuginfo=2 --remap-path-prefix` given to both builds) and warns only for
the code that runs in the build, never in kiln: built with the flags too
when there is no `--target`, never run by the training (WP-1862 at
26ebc59bf: 541 in build scripts and 646 in the proc macro
`kiln_snap_derive`, which WP-1308 predates); a mismatched one warns for
nearly every function of the crates it does not match (the release
profile used by a `--target aarch64-unknown-linux-gnu` build: 27,013
warnings across kiln's crates). The count reads each warning's code
unit (`build_script_build.*`, or a proc-macro crate's name) after taking
out cargo's color codes, which a CI runner's cargo prints (WP-1862: the
hosted measurement first counted all 1187 as kiln's). `cargo build -v`
shows `-Cprofile-use` on each `rustc` line.

**`llvm-profdata` discovery** (`pgo::ToolSearch`), before anything is
built. The tool's LLVM major must equal rustc's (`rustc -vV`, `LLVM
version:`; rustc 1.95 is LLVM 22.1.2): a profile is read only by the LLVM
that knows its format version. Candidates, in order, the first whose
`bin/llvm-profdata` exists and whose `--version` (`LLVM version <v>`) has
that major wins:

1. `--llvm-dir=<dir>`, else `$KILN_LLVM_DIR`: explicit, so a missing tool
   or another major is an error naming the directory, never a
   fall-through;
2. kiln's own LLVM search for that one major (spec/driver.md "LLVM
   discovery"): `llvm-config` and `llvm-config-N` on `PATH`, the layouts
   `/usr/lib/llvm-N`, `/usr/lib64/llvmN`, Homebrew's `llvm@N` and `llvm`,
   `/usr`, then `opt-N` and `clang++-N` on `PATH` (the installation above
   each);
3. `llvm-profdata-N`, then `llvm-profdata`, on `PATH`;
4. rustup's `llvm-tools` component,
   `<rustc --print sysroot>/lib/rustlib/<host>/bin/llvm-profdata`.

None usable is an error listing each candidate with why it was skipped.

**Training corpus and held-out set** (`pgo::TRAINING`, `pgo::HELD_OUT`;
`cargo xtask pgo-train --list`). Training: every bench entry except the
held-out ones and the groups that are runs of their own (`bench/pch`,
`bench/libcxx-slice`), 44 cases: `frontend/` less the held-out ones,
`oven/` less sort/string/table, all of `pp/` and `runtime/`, and the
reflection shapes S1-S5 and S9. Held out, never trained on so a bench run
against a plain build measures the gain honestly (19 cases):
`frontend/format-like-g`, `registration-wide-obj`, `templates`,
`templates_deep`, `templates_wide`, `templates-wide-g`, all four of
`overload/`, `oven/sort`, `sort-nohdr`, `string`, `table`, `table-nohdr`,
and the shapes S6, S7, S8, S10. A bench entry added later is in neither
list until it is put in one. The unit test `pgo::tests::corpus_lists`
checks that each listed file exists and that the lists are sorted,
duplicate-free and disjoint.

**What the steps need.** No network access. The checkout (with
`Cargo.lock`'s crates available, vendored or cached: the pre-step builds
xtask, whose third-party crates beyond `kiln_driver`'s `hashbrown` are
`signal-hook`, `signal-hook-registry`, `libc` and `errno`), the LLVM
installation kiln drives (with `llvm-profdata`, `clang++` and the libc++
headers some cases include), a libstdc++ (the system GCC's), and
`/usr/bin/time` if present (only for peak RSS). The one optional input
is GCC 16.1 (`KILN_GCC`, else `g++-16` on `PATH`): the reflection shapes
S1-S5 include its `<meta>`. Without it they are left out of the training
with a warning naming them; this is the only part of the corpus that
depends on the machine.

**Cost and gain** (WP-1308, WP-1862; docs/status/perf.md "PGO build"):
the instrumented build costs about one release build, the training under
a minute (29 s for the 44 cases at load 12), the final build one more
release build: 1m19s + 29 s + 1m55s against 1m46s for a plain release
build on the shared aarch64 box. The gain, on aarch64 and x86_64, is in
the status page: on the held-out set 0.82x the time of the plain build
on both (geometric means; memory 0.96x and 0.97x), measured at
26ebc59bf.

**Measuring it** (WP-1862). On one host: a plain `cargo xtask build`
copied aside, then `cargo xtask build --pgo`, then `cargo xtask bench
--compiler <PGO kiln++> --compare-with <plain kiln++> <held-out set>`
(5 runs; the ratios are PGO over plain of the medians). The sweep
workflow's opt-in `pgo` job (`.github/workflows/sweep.yml`: the `pgo`
dispatch input, or a push of a `sweep/pgo-<name>` branch, which runs it
alone) does exactly that on GitHub's x86_64 and aarch64 runners and
uploads the results JSON, the bench output and the build times as the
artifact `pgo-<arch>`. A failed entry fails the job, except an entry over
its `budget-time:` (spec/bench.md "Directives"): that is an absolute
budget for the development host, which a slower runner can break with
both binaries, and the comparison then has one entry fewer (the
geometric mean names the count).

## Feature table

`docs/STANDARD_SUPPORT.md` is the standard support table: what kiln
supports in each C++ standard, one `## ` section per standard (C++98/03,
C++11, C++14, C++17, C++20, C++23, C++26, C++26 reflection, DR-level papers,
kiln extensions), one row per core-language feature. Library papers are out
of scope (docs/status/libstdcxx.md and libcxx.md). Its C++23 and C++26 rows
are also the M4 language-feature checklist (`docs/ROADMAP.md` M4).
`docs/status/cxx-features.md`, its old home, is a pointer to it.
`cargo xtask features` checks it. The command is a manual / M4-gate check
and is not part of `ci`: a table edit, or a corpus change that breaks a
`done` row, would otherwise fail unrelated merges.

Format. A feature table is a Markdown table whose header row's first cell
is `Paper`. The file holds several, normally one per section. The header
cells must start with, in order, `Paper`, `Feature`, `` `__cpp_*` macro ``,
`Status`, `Corpus tests` and `Notes`. A `|---` separator follows, then one
row per feature, up to the first line that does not start with `|`. A `|`
inside a backtick span, or written `\|`, does not split cells. The macro
header cell may name the table's standard, as in `` `__cpp_*` macro (kiln
/ clang 22 at `-std=c++11`) ``; without one the table's standard is
`c++26`.

| Cell | Form |
|---|---|
| Paper | unique across the file: a paper `P<n>R<rev>` or `N<n>`, a core issue `CWG<n>`, a kiln work package `WP-<n>` (kiln extensions), or a stable name `[<name>]` (lower-case letters, digits and dots) for a baseline area with no single paper |
| Feature | free text |
| Macro | `—` (or `-`): the feature has no macro of its own; else one or more `` `__cpp_name` KILN / CLANG22 `` joined by `<br>`, each value either a number with an optional `L` suffix or `—` for undefined. KILN is `kiln++ -std=<std> -dM -E`, CLANG22 is `clang++-22 -std=<std> -dM -E -x c++ /dev/null`, `<std>` the table's standard |
| Status | `done` (kiln passes tests covering the core forms), `partial` (some pass, or some are `xfail(kiln)`, or only the macro is wrong), `todo` (nothing covers it, or kiln fails), `untested` (accepted, with no corpus test of its own yet) |
| Corpus tests | backticked paths relative to `tests/`, separated by `<br>` |
| Notes | free text: clang 19 values that differ, why a test is `xfail`, written reasons for C++26 rows clang 22 lacks |

A feature with no other test gets a probe at
`tests/conformance/<std-section>/<paper>-probe.cpp` (`strict-diags`,
`xfail(kiln)` with a reason while kiln fails, `xfail(clang)` naming the clang
versions that lack the feature).

Summary. The file opens with a summary table: a header row whose cells
start with `Standard`, `` `-std=` ``, `` `__cplusplus` ``, `done`,
`partial`, `todo` and `untested`, a separator, then one row per section.
The Standard cell is the section's `## ` heading text (or a link whose text
is); the `-std=` cell's first `` `-std=<std>` `` span is the standard its
`__cplusplus` cell is checked at (none, as in "every `-std=`", skips the
check; `—` in the `__cplusplus` cell does too); the four counts are the
section's rows by status. A section's rows are those of every table under
its heading with a `Status` header cell, including tables the row checks do
not read (the C++26 reflection section is per work package, first header
cell `WP`). `untested` rows count in neither `done` nor `partial`.

`cargo xtask features [--compiler <kiln++>] [--table <path>] [--summary]`
builds kiln unless `--compiler` or `--summary` is given, then checks:

- (a) every `done` row names at least one test, and every test any row
  names exists;
- (b) every test of a `done` row passes (`PASS`, not `XFAIL`) under kiln. All
  of them go through one `xtask test` runner invocation (`test::execute`, as
  `ci` uses it), with the default oracle and limits;
- (c) on a `done` row, each macro's kiln `-dM -E` value at the table's
  standard equals the table's CLANG22 value, unless clang 22 does not define
  the macro (a C++26 feature kiln ships ahead of clang 22, such as P3068; the
  row's Notes carry the written reason). On every row with a macro, each
  KILN value must be what kiln defines today at that standard, so that column
  cannot go stale;
- (d) the summary table matches the sections: a summary row with no section,
  a section with rows and no summary row, a duplicate row, or a count that
  differs is a failure naming the summary row (and, for a count, the field
  and both numbers); and each summary row's `-std=` gives kiln's
  `__cplusplus` value in its row.

It prints `N rows: D done, P partial, T todo, U untested`, then one `FAIL
<paper> (<feature>, line <n>): <problems>` line per failing row and one
`FAIL  summary row ...` line per summary problem. `--summary` parses the
file, prints each section's counts and runs the static half of (d) only (no
kiln build, no test run). Exit status: 0 for all rows and the summary
clean; 1 for a failing row or a stale summary; 2 for a usage, table parse
or infrastructure error. Parse errors name their line.

## Tests backing this spec

- `xtask/src/test/{directives,diag_parse,compare,pp_tokens,report}.rs` and
  `xtask/src/util/process.rs`: unit tests for the directive parser and
  placeholders, the diagnostic parser and matcher, normalization and diffs,
  token-wise pp comparison (token boundaries, digraphs, `<::`, line markers,
  mismatch reports), the parity report, `mem<=N` parsing, the `ci` retry
  decision, process timeouts, and the memory cap (`mem_limit_wraps_transparently`:
  arguments, working directory and environment survive the wrapper;
  `mem_limit_stops_a_hog`; `out_of_memory_signatures`;
  `kiln_stack_reservation`).
- Header-differential mode and `%{gcc_toolchain}` (WP-484):
  `meta_header_mode_selection` and `meta_header_directive_errors`
  (tests.rs), `gcc_install_prefix_is_four_up_from_install` and
  `gcc_args_drop_gcc_toolchain` (oracle.rs); the corpus run under
  `--meta-header` (docs/VALIDATION.md).
- GCC as the compiler under test (WP-1249): `gcc_under_test_gets_gcc_spelling`,
  `diag_directives_per_compiler_kind`, `gcc_links_unless_compile_only` and
  `gcc_under_test_stand_in_skip`
  (runner.rs), and `gcc_as_the_compiler_under_test` (tests.rs) over the
  `xtask/fixtures/gcc-compiler` fixtures with `fake-gcc/g++`, which
  rejects the kiln/clang-only flags as GCC does. WP-1261 adds
  `gcc_args_drop_clang_diagnostic_flags` (oracle.rs),
  `module_syntax_lines` and `gcc_preinclude_marker_dropped` (runner.rs) and the fixtures
  `pass-unit-two-step-run.cpp` (every GCC command of a `unit:` test in
  GCC's spelling, the link without `-Qunused-arguments` and with
  `-lstdc++exp` last), `pass-clang-warning-flags.cpp` and
  `skip-flags-gcc-rejects.cpp` (the flag probe), asserted by
  `gcc_as_the_compiler_under_test`.
- the oracle follows a clang under test (WP-1512):
  `the_oracle_follows_a_clang_under_test` (oracle.rs).
- clang oracle without crash reproducers (WP-1264):
  `clang_oracle_crash_is_retried_once` (tests.rs) checks every compile of
  the fake clang oracle carries `-fno-crash-diagnostics`.
- Debug-info sweep (WP-910): `debug_sweep_mode_selection` (tests.rs);
  the sweep of `tests/conformance` (docs/VALIDATION.md).
- `module:` / `min-oracle:` (WP-405): `module_and_min_oracle_directives`
  (directives.rs) and the `xtask/fixtures/basic` fixtures
  `pass-module.cpp` (two chained units precompiled by clang),
  `skip-min-oracle.cpp` and `error-module-missing-file.cpp`; the
  `tests/codegen/module_*.cpp` corpus under kiln and clang.
- `pch:` (WP-1023): `pch_directive` (directives.rs); the `pch-*`
  tests of `tests/conformance/cpp.include` under kiln and clang 19.
- `unit:` / `two-step` (WP-917): `module_and_min_oracle_directives`
  (directives.rs); the `module:`/`unit:` conformance tests
  (module.interface, module.unit, module.import, module.private.frag,
  module.global.frag, module.reach, basic.start.dynamic,
  basic.lookup.argdep; WP-986 converted the single-command ones) under
  kiln and under clang 19 and 22 as the compiler.
- `header-unit:` (WP-986): `header_unit_directive` (directives.rs);
  `module.import/header-unit.cpp` (a quoted header) and
  `header-unit-angled.cpp` (`<cstddef>`) under kiln and under clang 19
  and 22 as the compiler.
- Header-unit module files and `as-header-unit` (WP-1802):
  `as_header_unit_directive` (directives.rs); `module.import/`
  `header-unit-two-step.cpp` (a compile test that is `two-step`), the
  `run` tests above (two-step, so kiln builds `hu<N>.pcm`), and the
  `as-header-unit` tests `header-unit-external-defs*.cpp` and
  `header-unit-weak*.cpp`, under kiln (also `--bmi-kind 1`, `2` and
  `--bmi-image`, which select the `header-unit:` ones) and under clang 19
  and 22 as the compiler.
- `skip(clang)` (WP-258): `skip_clang_implies_oracle_none` and
  `skip_clang_rejected_without_golden` (directives.rs; the ERROR for
  `layout`/`vtables`/`ir`).
- `xtask/src/jobs.rs`: the jobserver client ("Jobserver") over a private
  fifo: `MAKEFLAGS` parsing (fifo form, last word wins, descriptor forms
  ignored), no jobserver is free, a non-fifo is rejected, concurrency stays
  within pool + implicit token with every byte returned
  (`bounds_concurrency_by_pool_plus_implicit`), a freed implicit token
  wakes a waiter, and a panicking holder returns its token.
- `xtask/src/jobs/signal_tests.rs` (WP-1436, "Jobserver", Signals): each
  test re-executes the test binary as a probe with a 4-token fifo and `sh`
  stub children in one phase, sends the signal, and asserts the probe died
  of it within 60 s, no stub is left running and the pool holds exactly 4
  tokens again: `sigterm_while_building`, `sigint_while_building`,
  `sigterm_while_building_with_stderr_reader_gone` (the WP-1436 hang),
  `sigterm_during_ci_step`, `sigint_during_ci_step_with_stderr_reader_gone`,
  `sigterm_while_tests_run`, `sigint_while_tests_run_with_stderr_reader_gone`,
  `sigterm_between_spawns`, `second_signal_exits_at_once` (a step ignoring
  SIGTERM, 60 s limits) and `stuck_cleanup_is_bounded` (the same with a 1 s
  bound). The limits come from `KILN_XTASK_TEST_SIGNAL_MS`, which only the
  test build reads. About 1 s in all.
- `xtask/src/gen_pp.rs`: the generator's options, and idempotent generation
  over a fake header tree.
- `xtask/src/test/tests.rs`:
  - `ci_gate_honours_failure_counts`: the `ci` corpus verdict ("CI") fails
    on a FAIL, XPASS or ERROR in either run or an infrastructure error, and
    allows XFAIL, SKIP and a skipped run;
  - `ci_retries_only_missing_crates_once`: the E0463 retry decision;
  - `shards_partition_the_test_list` (WP-692): `--shard` parsing and
    rejection, and shards 1/3, 2/3, 3/3 of `xtask/fixtures/ast/clang` are
    the round-robin slices of the unsharded list and together exactly it;
  - `memory_limits_with_a_fake_hog`: `xtask/fixtures/memory/` under
    `xtask/fixtures/fake-hog/kiln`, a shell script that identifies as kiln
    and allocates the MiB a `// fake-hog-mib: N` line asks for (with
    python3; the test is skipped without it), else compiles with clang. It
    covers KILLED for the compiler and for the test binary, `xfail` not
    masking it, `mem<=N` lowering the cap, kiln's stack allowance,
    `--mem-limit 0` keeping budgets, a lower `--mem-limit` reaching the
    test binary, a malformed `mem<=`, and the summary count;
  - the seed corpus (`tests/conformance`, `tests/differential` except the
    generated pp corpus, plus `tests/differential/pp/torture` and
    `tests/stress`) passes with
    clang, and every directive kind appears in it;
  - `xtask/fixtures/basic/` (one fixture per outcome, with the expected status
    as the file-name prefix) under the default `--timeout`, including program
    arguments, stdin and environment, a crash reported as its signal, the
    oracle's binary crashing, token-wise and text
    pp comparison, `pp-linemarkers` goldens, placeholders, `requires`
    (a pinned `--target` meeting and missing `target-x86_64`, and an unmet
    `clang>=999` under the clang compiler) and `verify` (`pass-verify.cpp`,
    and `fail-verify.cpp` with its report);
  - `xtask/fixtures/hang/`: the test's binary and the oracle's binary
    never ending, under a 1 s `--timeout` (the only fixtures with a short
    one: a hang times out whatever the host's load; WP-1888);
  - `xtask/fixtures/oven/`: `oven:` budgets, run against
    `xtask/fixtures/fake-kiln/kiln`, a shell script that identifies as kiln,
    compiles with clang and writes the canned report `<test>.oven.jsonl`
    (`@SRC@` replaced by the source path) to the `-fconstexpr-report=` path;
  - `xtask/fixtures/ast/clang/` (`expect: ast` under clang: golden match
    and mismatch, `ast-filter:`, diags, a compile error, a missing golden,
    a construct the normalizer rejects) and `xtask/fixtures/ast/kiln/` (the
    same under the fake kiln, which prints the canned dump `<test>.ast`
    with `@FILTER@` replaced by the `-fdump-ast-filter=` value, plus the
    `oracle: clang` comparison and `--no-oracle`);
  - `xtask/fixtures/dumps/kiln/` (`expect: layout` and `expect: vtables`
    under the fake kiln, which prints the canned dump `<test>.layout` or
    `<test>.vtables`, against clang: a match, a differing block, an xfail,
    an empty dump, a missing diag, an oracle that emits no vtable, and
    `--no-oracle`) and `xtask/fixtures/dumps/clang/` (the clang self-check:
    a layout with diags, a vtable with a thunk, a vtable clang does not
    emit, a compile error);
  - `xtask/fixtures/oracle/` (oracle disagreement, run with `--oracle
    clang++-22`; skipped if that compiler is absent);
  - `xtask/fixtures/skip-clang/` (`skip(clang)` implies `oracle: none`:
    under `fixtures/fake-kiln/kiln` the oracle is not run for a
    `skip(clang)` run test and is for its control; under clang it is a
    SKIP);
  - `debugger_fixtures_with_clang` (WP-814): `xtask/fixtures/debugger/`
    under clang++-19 (a gdb and lldb pass, a wrong value and an
    out-of-order check failing with the missing check named, and the
    ERROR for commands without checks; skipped without clang++-19, gdb or
    lldb-19);
  - `--filter`;
  - `--update` (pp and ast goldens).
- WP-911: `ice_directives` (directives.rs) and
  `crash_diagnostics_off_takes_the_last_flag` (runner.rs);
  `tests/conformance/driver.crash/` runs `expect: ice` end to end.
- `debugger_directives` (directives.rs) and
  `debugger_checks_match_successive_lines_in_order` (runner.rs): the
  `gdb:`/`lldb:` directive errors and the in-order check matcher;
  `tests/debuginfo/` under kiln and clang++-19.
- WP-816: `debugger_gate_skips_or_errors_without_the_debugger` (runner.rs:
  a missing gdb or lldb is a SKIP, an ERROR under the requirement, and
  only for the debuggers the test drives), `debugger_corpus_is_a_default_root`
  (tests.rs: `KILN_REQUIRE_DEBUGGERS` parsing, `tests/debuginfo` among the
  default roots) and `ci_tests::workflow_installs_the_debuggers` (main.rs:
  `ci-lane.yml` sets the requirement and every corpus job installs the
  debuggers in a `SYS_PTRACE` container).
- WP-1034: `sanitizer_runtime_gate_skips_or_errors` (runner.rs: a missing
  sanitizer runtime is a SKIP, an ERROR under
  `KILN_REQUIRE_SANITIZER_RUNTIME=1`), `mangle_corpus_paths` (tests.rs:
  which `xtask test` paths run the mangling differential), `cargo_steps`
  (main.rs: the unit step's split-out differential command), and
  `ci_tests::workflow_installs_the_sanitizer_runtimes` /
  `workflow_runs_the_cmake_modules_e2e` (main.rs).
- `kiln_abi::dump_blocks` unit tests: block splitting, builtin records,
  `Original map` and duplicate construction vtables.
- `tests/conformance/class.derived/layout-*.cpp` and
  `tests/conformance/class.virtual/{layout,vtable}-*.cpp`: the modes on the
  conformance corpus.
- `xtask/src/clang_ast/tests.rs`: the clang-ast normalizer (the mapping
  table covers M2, unknown kinds and keys are errors, the spec example,
  values, strings, the side channel), over `xtask/fixtures/ast/normalize/`;
  the clang 22 attributes it ignores
  (`clang22_template_instantiation_pattern_is_ignored`,
  `clang22_lambda_has_explicit_parameters_is_ignored`).
- `xtask/src/oracle.rs`: "Oracle toolchain": `KILN_ORACLE` unset/empty is
  19, `19` and `22` name their tools and directories, anything else is an
  error naming both values.

- `xtask/src/features.rs` tests: the feature-table parser over
  `xtask/fixtures/features/{good,bad}.md` (cells, macro forms, code-span
  pipes, multi-macro cells, `untested`, line-numbered errors), the static
  checks (a) and (c) at each table's standard, the summary check (d)
  (`summary_matches_sections`, `summary_parse_errors`: an inline table
  with a stale count, a renamed section and a wrong `__cplusplus`), and a
  check that `docs/STANDARD_SUPPORT.md` itself parses, names only existing
  tests and has a summary matching its sections.
- `xtask/src/gcc_tests/{tcl,target,dg,xfile,outcome,torture}.rs` and
  `mod.rs` (WP-1277): "GCC testsuites": Tcl lists, braces, quotes and
  `string match` globs; selectors and the effective-target table
  (`__OPTIMIZE__` per option set); directive extraction and per-option
  `dg-options`/`dg-additional-options`/`dg-skip-if`/`dg-require-effective-
  target`/xfail resolution; `.x` evaluation (flag replacement, `return 1`,
  `torture_eval_before_compile`); the result classes and cluster keys;
  the option parser, planning and skip collapsing, the TSV, the summary and
  its splice; flag units. All on inline text, so `ci` needs no checkout.
- `xtask/src/gcc_tests/{expect,gxx}.rs` (WP-1443): "The g++.dg runner":
  line references (`.`, `.-N`, `.+N`, 0), expectation selectors (`target`,
  `xfail`, `c++NN` at the case's standard), `dg-excess-errors` /
  `dg-prune-output`, the diagnostic reader (line, severity, other files),
  the judge (rejects-valid, accept-invalid, missed and extra lines,
  warnings and messages accepting an error); `g++-std-flags` (floors,
  fill order, `gnu++` with `dg-options`, a test's own `-std=`,
  `GXX_TESTSUITE_STDS`); case planning (torture option sets and
  `cxx_default`, modules and input-kind skips, additional sources,
  `dg-shouldfail`, `dg-final` kinds); the buckets, clusters, summary and
  TSV; the `c++NN` effective targets.

## Open questions

- **kiln's silent exit 70.** When kiln cannot reserve its compiling
  thread's stack (a cap below the reservation, without the harness's
  allowance), it exits 70 without a diagnostic
  (`crates/kiln_driver/src/main.rs`), so the harness cannot tell it from
  other failures. kiln should say what it failed to reserve.
- **Resident vs virtual.** The cap bounds address space, not resident
  memory, so a budget includes untouched reservations (allocator arenas,
  thread stacks). Peak RSS is measured by `xtask bench`, not bounded.

- **Located `oven:` budgets in headers.** A location names a line of the test
  file; there is no way yet to bound an evaluation located in an included
  file, other than through the total.
