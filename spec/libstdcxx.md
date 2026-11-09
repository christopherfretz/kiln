# libstdc++ header status: `cargo xtask libstdcxx`

Status: implemented (WP-188 phase 1). Code: `xtask/src/libstdcxx/`. This is
the contract for the per-header status of GNU libstdc++ under kiln: each
public header the installed libstdc++ ships is compiled alone, with kiln and
with clang++-19 as the reference, and the result goes to
`docs/status/libstdcxx.md`. It is the first phase of the libstdc++
consumption track (ROADMAP M4/M6). It is informational: it is **not** part
of `cargo xtask ci` (`spec/harness.md` "CI"), and a header kiln rejects does
not fail the command. Phase 2 (running `testsuite/**/*.cc` files) is not
specified yet.

## Command line

```
cargo xtask libstdcxx [options]
```

| Option | Meaning |
|---|---|
| `--std <std>` | `c++26` (default), `c++23`, `c++20` or `c++17`. Each standard has its own region of the report. |
| `--headers <dir>` | The libstdc++ include directory to enumerate. Default: the newest `/usr/include/c++/<version>` whose name is dot-separated numbers, compared numerically (Debian's `libstdc++-N-dev` layout; `v1`, libc++'s, never matches). Both compilers still find their headers through their own `-stdlib=libstdc++` search (`spec/driver.md` "Header search"); the option only chooses what to enumerate. |
| `--gcc-toolchain <dir>` | Passed to both compilers as `--gcc-toolchain=<dir>` (spec/driver.md "Toolchain"), so they take libstdc++ from that GCC installation; the default `--headers` becomes the newest `<dir>/include/c++/<version>`. For GCC 16.1: `--gcc-toolchain $KILN_GCC16_ROOT/usr/local` (docs/VALIDATION.md "GCC 16.1 header sweep"; WP-525). |
| `--profile <p>` | The cargo profile the implicit build uses: `debug`, `harness` (default) or `release` (`spec/libcxx.md` "Build profiles"). |
| `--compiler <path>` | kiln binary. Default: `cargo build --profile <p> -p kiln_driver`, then `kiln++`. The report's command names `--profile <p>`, or `--compiler <kiln++>` when this is given. |
| `--clang <path>` | The reference compiler. Default `clang++-19`. If it cannot report a version, a warning is printed and the run continues without a reference column. |
| `--no-clang` | Skip the reference compiler. |
| `--filter <glob>` | Keep only headers whose name matches the glob in full (`*` any run, `?` one character, everything else literally). |
| `--list` | Print the selected headers and exit. Nothing is compiled. |
| `-j, --jobs <n>` | Headers in parallel. Default 2. Each compile holds a jobserver token (`spec/harness.md` "Jobserver"), so the shared pool can lower it. |
| `--timeout <secs>` | Per-compile limit. Default 120, or 300 with `--profile debug`. |
| `--mem-limit <MiB>` | Address-space cap per compile (`spec/harness.md` "Resource limits"). kiln gets its stack reservation on top. Default 8192. `0` means no cap. |
| `--report <file.md>` | Write this standard's generated region of the status page (see "Report"). |
| `-v` | Print every header's result, not only kiln's non-ok ones. |

## Header selection

The public headers are the regular files (or links to them) **directly** in
the include directory whose name has no `.`, sorted by name. This takes the
C++ headers (`<vector>`, `<print>`, ...) and the `<cxxx>` C compatibility
headers (`<cstdio>`, `<ccomplex>`, ...). It leaves out every subdirectory
(`backward/`, `bits/`, `debug/`, `decimal/`, `experimental/`, `ext/`,
`parallel/`, `pstl/`, `tr1/`, `tr2/`: implementation detail, extensions,
technical specifications, parallel mode) and the `.h` and `.tcc` files
(`<cxxabi.h>`, `<math.h>`, ...). For Debian's GCC 14 this is 109 headers.

## Commands

Each header `h` (index `i`) gets the scratch directory
`<target>/xtask-libstdcxx/run-<pid>-<nanos>/h<i>/`, holding `t.cpp` with the
single line `#include <h>`. Both compiles run with the cwd and `TMPDIR` set to
that directory, which is removed afterwards:

```
kiln++     -resource-dir <repo>/resource -std=<std> -stdlib=libstdc++ -fsyntax-only -ferror-limit=0 t.cpp
clang++-19                               -std=<std> -stdlib=libstdc++ -fsyntax-only -ferror-limit=0 t.cpp
```

Each compile is measured like a `bench` compile (`util::process::run_measured`:
wall clock from spawn to reap, peak RSS via GNU time `%M`), under the timeout
and the memory cap.

## Outcomes

Each compile ends in one of:

| Status | Meaning |
|---|---|
| `ok` | Exit status 0. |
| `fail` | A non-zero exit with diagnostics. Recorded: the number of lines carrying an `error:` (or `fatal error:`) message and the first such line, with the include directory's and the scratch directory's path prefixes removed. With no `error:` line, the first non-empty stderr line stands in. |
| `crash` | A panic (kiln's `internal compiler error:` banner or `panicked at`, keyed as in `spec/libcxx.md` "Clusters"), LLVM's crash banner, or death by a signal (exit code ≥ 128). kiln runs with `-fno-crash-diagnostics`, so a crash writes no reproducer. |
| `timeout` | Ran past `--timeout`; the process group is killed. |
| `killed` | Hit the memory cap (`out_of_memory`, `spec/harness.md`). |
| `error` | The compiler could not be started. |

A header the reference compiler does not accept is reported as such (its
column says `**rejects**` with the first error) and kiln's result on it is
not counted as a kiln failure: it is left out of the clusters and counted
separately in the summary.

**Exit status:** 2 on a usage error or a tool error (no include directory,
no header selected, the kiln build failed, the report could not be written,
or any compile ended in `error`); 0 otherwise, whatever kiln's results are.

## Report

The console prints each header where kiln is not `ok` as
`[k/n] <h>: <status>`, then the summary line and the ten largest clusters.

`--report <file.md>` writes one region per standard, between
`<!-- libstdcxx:<std>:begin -->` and `<!-- libstdcxx:<std>:end -->`.
A run replaces its own standard's region and leaves the rest of the page
alone, so hand-written triage outside the markers survives. A page without
that standard's region gets it appended (a new page is the region alone).
The region contains:
- A heading `` ### `-std=<std>` `` and the **summary line**: `<std>: N of M
  headers ok with kiln (F fail, C crash, T timeout, K killed; R on headers
  clang++-19 rejects); clang++-19 accepts A of M; kiln wall p50 / p95, peak
  RSS p50 / p95; clang++-19 wall p50 / p95, peak RSS p50 / p95`. The
  percentiles are nearest-rank over all headers.
- A run table: kiln version and commit, the reference compiler's version,
  `_GLIBCXX_RELEASE` and `__GLIBCXX__` (from `-E -dM` of `<version>`, by
  the reference compiler, else kiln) with the include directory, the header
  selection, the command, the shared flags, the limits, the RSS source and
  the wall time.
- **Headers**: one row per header with the columns `header`, `kiln`
  (status), `errors`, `first error` (kiln's; the crash key for a crash),
  `clang++-19` (`ok`, `**rejects** (n): first error`, `**timeout**`, ...,
  or `not run`), `kiln wall`, `kiln RSS`, `clang wall`, `clang RSS`.
- **Error clusters**: kiln's `error:` messages over the non-ok headers the
  reference accepts, normalized as in `spec/libcxx.md` "Clusters" (no stage
  prefix; a crash, timeout or memory kill is the header's first key). Each
  row has its **first** count (headers whose first key it is), its **any**
  count, and the headers it is first for (else an example header). Ranked
  by first, then any, then text; the top 30 are listed.

The hand-written "Clusters" section of `docs/status/libstdcxx.md` (root
causes, minimal repros under `tests/conformance/**/libstdcxx-*.cpp`, proposed
work packages) lives outside the markers.

## Tests backing this spec

Unit tests in `xtask/src/libstdcxx/*.rs`, with the fixture include tree
`xtask/fixtures/libstdcxx/include/c++/{9,14,v1}`:
- `mod.rs`: options; header enumeration (extension-less files only, every
  skipped directory present in the fixture, `.h`/`.tcc` left out); the
  numeric choice of the default version directory; globs.
- `run.rs`: outcome classification, error counting, path shortening, crash
  keys, memory kills, timeouts.
- `report.rs`: percentiles, the summary line, page rendering (rows, the
  reference-rejects column, clusters excluding reference-rejected headers),
  per-standard region splicing.

The repros of the triaged clusters are conformance tests
(`tests/conformance/**/libstdcxx-*.cpp`, `xfail(kiln)` until fixed) with the
usual clang++-19 oracle.

## Open questions

- **GCC 15/16.** Debian ships GCC 14's libstdc++ today. M6's reference is
  GCC 16.1; `--gcc-toolchain` (WP-525) makes `--headers` and the driver's
  GCC detection agree on an unpacked GCC 16.1 tree.
- **Phase 2.** Driving `testsuite/**/*.cc` with their `{ dg-do run|compile }`
  markers needs the libstdc++ testsuite sources (not installed by Debian).
