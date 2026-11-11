# Bench: `cargo xtask bench`

Status: implemented (WP-004; constexpr counters and per-entry thresholds
WP-006; `perf.md` merging WP-006b; the `libcxx-slice` entry WP-317; the `pch` group WP-1294). Code: `xtask/src/bench/`, with shared pieces in
`xtask/src/util/` (`process.rs` measured runs, `header.rs` the `//@` scanner,
`json.rs` the record format). Policy background is `docs/VALIDATION.md`
"Performance tracking"; this file is the normative contract for the harness,
for entry authors, and for anything that reads its records.

## Command line

```
cargo xtask bench [options] [paths...]
```

| Option | Meaning |
|---|---|
| `--compiler <path>` | Compiler to measure. Default: `cargo build --release -p kiln_driver`, then `<target>/release/kiln++` (a debug kiln says nothing about performance). |
| `--baseline <dir>` | Baseline directory. Default `bench/baselines`. Relative paths are taken from the repo root. |
| `--record` | Write the measurements as the new baselines (see "Recording"). With `--compare-with`, also merge the ratio table into `docs/status/perf.md`. |
| `--compare-with <path>` | Also measure this compiler on the same entries and print the ratio table: `clang++-22` (the libc++ slice's oracle, the M4 reference), or `clang++-19` for the history of `docs/status/perf.md`. |
| `--filter <substr>` | Keep only entries whose id (`<group>/<name>`) contains `<substr>`. |
| `--label <name>` | `bench/pch` only: with `--record`, write region `<name>` of `docs/status/pch.md` and no baselines ("The pch group"). Default `main`. |
| `--runs <N>` | Runs per entry, N >= 1. Default 5. |
| `--threshold-time <PCT>` | Allowed wall-clock regression, percent, >= 0. Default 5. |
| `--threshold-mem <PCT>` | Allowed peak-RSS regression, percent, >= 0. Default 2. |
| `--threshold-steps <PCT>` | Allowed constexpr `steps` regression, percent, >= 0. Default 0. |
| `--threshold-bytes <PCT>` | Allowed constexpr `bytes`/`peak_bytes`/`peak_arena`/`frozen_bytes`/`static_bytes` regression, percent, >= 0. Default 2. |
| `--mem-limit <MiB>` | Address-space cap of every measured compile. Default 8192; `0` disables it. See "Resource limits". |
| `--scaling <shape>` | Instead of the corpus, the scaling sweep of one reflection shape (`s1`..`s6`); see "Scaling sweeps". |
| `--n <N>` | The sweep's N (default: the shape's default N); only with `--scaling`. |
| `--external <dir>` | Instead of the corpus, the measured never-crash run of a directory outside the repository; see "External corpora". |

A `--threshold-*` option overrides the same entry's `threshold-*:` directive
(below), which overrides the default. Options apply to every selected entry.
| `-v, --verbose` | Print each compiler command. |
| `-h, --help` | Usage. |

`--scaling` and `--external` exclude each other and take no paths,
`--record` or `--compare-with`.

`--flag=value` is accepted for every option that takes a value. `paths` are
files or directories under the bench root (default: all of `bench/`); a
relative path that does not exist from the current directory is taken from
the repo root. `bench/libcxx-slice` (or its `slice.list`) selects the libc++
slice; the default `bench/` does not (see "The libcxx-slice entry").

Exit status:

| Code | Meaning |
|---|---|
| 0 | Every entry was measured, and none regressed beyond a threshold (or `--record` was given). |
| 1 | An entry failed (directive error, compile error, crash, timeout, a bad or over-budget constexpr report) for either compiler, or an entry regressed and `--record` was not given. |
| 2 | Usage or infrastructure error: bad option, no entries selected, a compiler that cannot run `--version`, an unreadable baseline file, an unwritable output. |

## Entries

An entry is `bench/<group>/<name>.cpp`. Its id is `<group>/<name>`; `name` may
contain further `/` for nested directories. Discovery walks the given paths,
keeps `.cpp` files, skips names starting with `.`, directories named
`Inputs`, and the top-level `bench/results/` and `bench/baselines/`. A file
directly in `bench/` (no group) is an error. Entries are sorted by id and run
**serially**, one compiler process at a time, so they never compete with each
other for the CPU.

Every entry is self-contained: no includes outside the standard library, no
generated inputs other than a `generate:` entry's own ("Generated entries").
An entry may include itself (by its literal file name, not
`__FILE__`) to repeat a workload; it stays one file. An entry measures one
thing; it should take 0.5–3 s with the reference compiler: on a loaded host,
~0.1 s entries are dominated by process start-up and scheduling noise.
Constant-evaluation entries end in `static_assert`s so that a compiler which
skips the work cannot look fast.

Groups in the seed corpus: `pp` (preprocessing: the libc++ include graph with a
header-style configuration layer, X-macro tables, Boost.PP-style recursive
expansion), `oven` (constant evaluation), `frontend` (template-heavy parsing
and semantic analysis), `overload` (overload resolution: named calls over
large overload sets, constructor and conversion selection;
spec/sema-overload.md §11), `runtime` (`mode: run`: the speed of the generated
program, e.g. a `std::generator<int>` iterated 10M times). `reflect` (the
reflection shapes, "Reflection shapes") is generated and, like the libc++
slice, measured only when named (`bench/reflect` or one of its files): the
default `bench/` skips it.

## Generated entries

An entry with a `generate: <generator> <params>` directive is measured on a
source generated at bench time, so a multi-megabyte input never lands in the
repository; the committed file is only its directive header and a comment.
The harness writes the text to `<scratch>/generated/<id>.cpp` (the run's
scratch directory, removed at the end) and hands that path to every
compiler, with the entry's other directives. The text is deterministic, and
the record's `source_hash` covers the entry file followed by the generated
text, so a generator change makes the baseline `advisory`. Generators
(`xtask/src/bench/generate.rs`):

| Generator | Source |
|---|---|
| `nested-dup <N> <DEPTH> <WIDTH>` | The repeated nested template-id TU of docs/status/perf-history.md "WP-879" (byte-identical to its `gen.py N DEPTH WIDTH 1`): `N` reflection-keyed specializations of a function template and of a class template's static member, each keyed by a template-id `DEPTH` levels deep with `WIDTH` arguments per level, every level repeating the previous one twice. 1 <= N <= 100000, DEPTH <= 10, 2 <= WIDTH <= 16. Needs `-freflection`. |

`bench/frontend/nested-template-ids-dup.cpp` (`nested-dup 200 6 4`, 1.3 MB;
WP-919) is the one generated entry. Its depth stays at 6 to keep the run
short; the 5.1 MB depth-8 TU of docs/status/perf-history.md "WP-919" has the same
slope.

## The libcxx-slice entry

`bench/libcxx-slice/slice.list` names ~50 heavy libc++ tests, one path per
line relative to the `libcxx/test` directory of the llvmorg-22.1.8 checkout
`cargo xtask libcxx` uses (`libcxx::M4_TAG`); blank lines and `#` comments are
skipped, and an absolute path, `..`, an empty component, a non-`.cpp` path, a
repeated path or an empty list is a usage error (exit 2). Each listed test is
an entry with id `libcxx-slice/<path without .cpp>` and mode `syntax`; it has
no directives (the file is a libc++ test, not a bench entry). `--filter`
applies to these ids.

- **Selection.** Only an explicit `bench/libcxx-slice` or
  `bench/libcxx-slice/slice.list` path selects the slice, alone or beside
  other paths; the default run over `bench/` does not, because it needs the
  libc++ 22 checkout and takes minutes, which every regression run would pay.
- **Flags.** Each compiler compiles each test exactly as the libc++ runner
  would at `-fsyntax-only` (`spec/libcxx.md` "Commands"): `libcxx::syntax_compiles`
  builds the runner's base flags, the test's `ADDITIONAL_COMPILE_FLAGS`,
  feature-dependent flags and supported warning flags for that compiler
  (clang gets `-nostdinc++ -isystem` of the libc++ 22 headers, kiln its
  `-fclang-compat=22` and `-resource-dir`), with `KILN_LLVM_DIR` set; a
  `.verify.cpp` test keeps its `-verify` flags. A test the runner would not
  compile at `-fsyntax-only` (UNSUPPORTED on this host, XFAIL, a `.sh.cpp` or
  a compile-fail test) fails the entry with the reason; the list holds none.
  Neither the constexpr nor the instantiation report flag is passed (the
  runner passes neither). The recorded `flags` show paths under the repo
  root relative to it and toolchain paths from `.toolchain/` on, so a
  worktree and the main checkout record the same string. The oracle must be
  clang 20 or later: libc++ 22's `<__configuration/compiler.h>` warns
  "Libc++ only supports Clang 20 and later", and the runner's `-Werror`
  makes that an error, so with `--compare-with clang++-19` every clang side
  would fail. `xtask bench` therefore refuses, with an error (exit 2) before
  any setup or measurement, a clang `--compare-with` or `--compiler` older
  than clang 20 or whose `--version` names no version, when the slice is
  selected (`slice::check_clang_floor`, WP-487); kiln and other kinds are not
  held to the floor, and without the slice clang 19 stays a fine oracle. The
  slice's oracle is `clang++-22`.
- **Measurement.** As any entry: serial, one compiler process at a time,
  `--runs` runs, the memory cap with kiln's stack reservation added. The
  1-minute load average is sampled before each test.
- **Not judged.** Slice entries are never compared with baselines and never
  recorded in `bench/baselines/` (their status is `not judged`, counted in
  the summary); a failed compile still fails the run. Their meaning is the
  ratio to the oracle measured in the same window: an absolute millisecond
  baseline from another window moves with the load of this shared host
  (5–30% at load 8–27, "Regression policy"), and ~50 tests at that noise
  would fail some run every day. The ratio absorbs most of the load because
  both compilers run back to back on each test, so it is what is recorded
  and tracked, by hand, release to release (`docs/status/perf.md`).
- **Report.** With `--compare-with` the printed output has a per-test table
  (both medians, the time ratio, both peak RSS values, the memory ratio), the
  load average (first, mean, max of the samples), and the aggregate ratios
  over the tests both compilers measured: time median, geometric mean,
  nearest-rank p90 and maximum with the test that has it; memory likewise. A
  failed side is `FAIL` in the table and left out of the aggregates, whose
  count says how many tests they cover. These rows are not in the ordinary
  ratio table or its geometric means.

## The pch group

`bench/pch` measures kiln's precompiled headers (spec/driver.md, "The
declaration image") against clang's on fixed header sets
(`xtask/src/bench/pch.rs`, WP-1294). It is a run of its own:

- **Selection.** Only an explicit `bench/pch` path selects it, and it must
  be the only path (otherwise exit 2). The default run over `bench/` skips
  the directory, which is reserved like `results/` and `baselines/`, so its
  `.cpp` files are never ordinary entries. Naming a file under it is a usage
  error. `--filter` narrows the run by entry id.
- **Compilers.** `--compiler` must be kiln (the default release `kiln++`).
  The comparison compiler is `--compare-with`, default `clang++-19`, and it
  must be clang. Both measure every variant they have, back to back.
- **Cases.** Each case is a header set `bench/pch/<case>.h` and a
  translation unit `bench/pch/<case>.cpp` (`.c` for C). The unit has no
  `#include` of its own. It uses what the set declares, so a consumer that
  skipped the header cannot compile it.

  | Case | Header set | Flags |
  |---|---|---|
  | `gnu` | `<bits/stdc++.h>` (libstdc++ 14) | `-std=c++26` |
  | `lcx` | the 106 public libc++ 19 headers of docs/status/pch-image-census.md, one `#include` each | `-std=c++26 -stdlib=libc++` |
  | `mid` | `<algorithm>`, `<map>`, `<string>`, `<vector>` (libstdc++ 14) | `-std=c++26` |
  | `c` | `<stdio.h>`, `<stdlib.h>`, `<string.h>`, `<zlib.h>` | `-std=c17`, a C header (`-x c-header`) and a C unit (`-x c`) |

- **Variants.** Each case has the entries `pch/<case>/<variant>`. The
  baseline name is `<case>/<variant>` in `bench/baselines/pch.json`. They
  run in this order, the build first:

  | Variant | Mode | kiln | clang |
  |---|---|---|---|
  | `build` | `pch-build` | `-fkiln-pch-image -x c++-header <case>.h -o <pch>` | `-x c++-header <case>.h -o <pch>` |
  | `none-syntax` | `syntax` | `-include <case>.h -fsyntax-only` | the same |
  | `none-obj` | `obj` | `-include <case>.h -O0 -c -o <obj>` | the same |
  | `load-syntax` | `syntax` | `-fkiln-pch-image=load -include-pch <pch> -fsyntax-only` | `-include-pch <pch> -fsyntax-only` |
  | `load-obj` | `obj` | `-fkiln-pch-image=load -include-pch <pch> -O0 -c -o <obj>` | `-include-pch <pch> -O0 -c -o <obj>` |
  | `replay-syntax` | `syntax` | `-fno-kiln-pch-image -include-pch <pch> -fsyntax-only` | none |
  | `replay-obj` | `obj` | `-fno-kiln-pch-image -include-pch <pch> -O0 -c -o <obj>` | none |

  Every command starts with the case's flags and ends with the unit (the
  build ends with `-o <pch>`). `<pch>` is `<case>.<kind>.pch` and `<obj>`
  is `<case>.<kind>.o` in the run's scratch directory under the target
  directory. Neither is ever `/dev/null`. The recorded `flags` keep the
  `<pch>` and `<obj>` placeholders, so the recorded flags are the same in
  every checkout. `mode: pch-build` exists only for these records. A
  bench entry's `mode:` directive cannot name it. A consumer selected
  without its build (`--filter`) builds the PCH once, unmeasured.
- **Measurement.** As any entry (`--runs`, default 5, median; the memory
  cap). No constexpr or instantiation reports are requested. The 1-minute
  load average is sampled at the start of the run and of each case.
- **The path taken.** After its variants, each case runs kiln twice more,
  unmeasured:
  - the build with `-v`, to a separate PCH, for the producer's note
    (`consumers use its declaration image (N records)` or `consumers
    replay the header: <why>`);
  - `load-syntax` with `-v`, for the consumer's note (`declaration image
    (N records)` or `is replayed: <why>`).

  The consumer's note gives the path: `image`, `replay`, or `unknown` when
  there is no note. When the image is taken, `-fkiln-pch-image=check` is
  run at `-fsyntax-only` and at `-O0 -c`. It compiles both paths in one
  process and fails on any difference, and that failure fails the run as
  an image/replay difference, which is a bug in the image. This is a smoke
  check of the measured cases only. The correctness check is the harness's
  `--pch-diff` (spec/harness.md "PCH differential").
- **Judged.** kiln's entries are judged against `bench/baselines/pch.json`
  as in "Regression policy", but wall time is not judged unless
  `--threshold-time` is given. Memory (default 2%) is the signal, because
  wall time on the shared host moves about 1.4x with its load. clang's
  side is never judged or recorded; a failed clang compile still fails the
  run. The printed ratio table covers the variants both compilers have.
- **Recorded.**
  - The results file (`bench/results/<stamp>.json`) is the ordinary
    record with both compilers' runs, plus a `pch` key:
    - `label` and `commit` (`git rev-parse --short=9 HEAD` of the root);
    - `cases.<case>`: `load_avg_1m`, `pch_bytes_kiln`, `pch_bytes_clang`,
      `path`, `producer_note`, `consumer_note`, and `check`, which is
      `null` when not run, `"equal"`, or the first error.

    Readers of results ignore the key ("Records").
  - `--record` writes kiln's baselines into `bench/baselines/pch.json`.
    It also rewrites the `main` region of `docs/status/pch.md`.
  - `--record --label <name>` rewrites only region `<name>` and records no
    baselines. It is for a run with another kiln, for example a branch's
    build passed by `--compiler`. A label is letters, digits and `-_.+/@`.
  - With `--filter`, `--record` still writes the baselines but leaves the
    page alone, because a region holds every case.
- **The page region.** It is the text between `<!-- xtask bench pch
  <label>: begin ... -->` and `<!-- xtask bench pch <label>: end -->`. The
  generator replaces it in place, or adds a new label after the last
  region (or at the end of a page that has none). Everything outside the
  markers is prose and is kept verbatim.

  A region has a header: the date, the results stamp, the kiln version and
  commit, the clang version, the host, the run count, the RSS source and
  each case's load average. Then, per case:
  - the path line with both notes and the check result;
  - both PCH file sizes;
  - a table of every variant in page order (none, build, load, replay),
    with kiln and clang seconds and MiB and the kiln/clang time and memory
    ratios;
  - the PCH load over no PCH time ratio for each compiler;
  - the case's command lines.
- **Cost.** About 4 minutes at 5 runs on the reference host, and about
  1 minute at `--runs 1`. It is not part of `cargo xtask ci` and runs in
  no CI workflow. Run it by hand when the PCH writer or consumer changes,
  and before a release:

  ```sh
  cargo xtask bench bench/pch                   # judge against pch.json
  cargo xtask bench bench/pch --record          # new baselines + region `main`
  cargo xtask bench bench/pch --record --label wp-NNNN@<sha> \
      --compiler <that tree's target>/release/kiln++
  ```

**Reproducing one number by hand.** Run from the repo root, with `T` a
scratch directory (`T=$(mktemp -d)`), and time each command with
`/usr/bin/time -f "%e s %M KB"`. For example, the `gnu` case's PCH load
at `-O0 -c`:

```sh
kiln++ -std=c++26 -fkiln-pch-image -x c++-header bench/pch/gnu.h -o $T/gnu.kiln.pch
kiln++ -std=c++26 -fkiln-pch-image=load -include-pch $T/gnu.kiln.pch -O0 -c -o $T/gnu.o bench/pch/gnu.cpp
clang++-19 -std=c++26 -x c++-header bench/pch/gnu.h -o $T/gnu.clang.pch
clang++-19 -std=c++26 -include-pch $T/gnu.clang.pch -O0 -c -o $T/gnu.o bench/pch/gnu.cpp
```

Add `-v` to a kiln command to see its PCH note. Every region of
docs/status/pch.md lists each case's full command lines
(`pch::command_line`).

## Directives

The header is the `//@ key: value` lines before the first line of code, in
the same syntax as `cargo xtask test` (`spec/harness.md` "Directives"). The
scanner is shared; the keys are not.

| Key | Meaning |
|---|---|
| `std: <std>` | Language standard, one of the C++ values accepted by `cargo xtask test`. Default `c++26`. |
| `args: <flags>` | Extra compiler flags, whitespace-split; may repeat (concatenated in order). |
| `mode: syntax\|pp\|emit-llvm\|obj\|run` | What the compiler does (below). Default `syntax`. |
| `oven: [<line>[:<col>]] <counter><=N ...` | A constexpr budget, as in `cargo xtask test` (`spec/harness.md` "kiln-specific tests"); may repeat, once per location. Checked on every kiln run; a breach fails the entry. An error with `mode: pp` or `mode: run`. |
| `threshold-time: <PCT>` | This entry's allowed wall-clock regression (default 5). |
| `threshold-mem: <PCT>` | This entry's allowed peak-RSS regression (default 2). |
| `threshold-steps: <PCT>` | This entry's allowed constexpr `steps` regression (default 0). |
| `threshold-bytes: <PCT>` | This entry's allowed constexpr `bytes`/`peak_bytes`/`peak_arena`/`frozen_bytes`/`static_bytes` regression (default 2). |
| `generate: <generator> <params>` | Measure a source generated at bench time instead of the file's own body ("Generated entries"). An unknown generator, bad parameters or a repeat is an error. |
| `budget-time: <ms>` | An absolute wall-clock budget for kiln, in milliseconds (a positive number): kiln's **fastest** run must not take longer (load only ever adds time, as in the time-regression rule). Checked on every kiln run, `--record` included, so re-recording cannot absorb a slowdown past it; a breach fails the entry (`over the time budget: ...`) and the entry is not recorded. Other compilers are not held to it. Host-specific like the baselines: set it at ~1.5x the recorded kiln median on the recording host, next to the number it guards (`bench/pp/headers.cpp`, WP-487). |
| `xfail(kiln): <reason>` | The entry is expected to fail under kiln; the reason names the owning WP. A kiln failure then counts as `xfail` (row status `xfail (<reason>)`, summary `, N xfail`), not as a failure; a kiln compile that succeeds fails the entry (`XPASS: … remove the marker`). Other compilers measure the entry as usual. A missing reason or a repeat is an error. |

| Mode | Flags |
|---|---|
| `syntax` | `-fsyntax-only` |
| `pp` | `-E -o /dev/null` |
| `emit-llvm` | `-S -emit-llvm -o /dev/null` |
| `obj` | `-c -o /dev/null` (optimiser and backend included; the `-O` level is the entry's `args`, WP-641) |
| `run` | none (see below) |

Output goes to `/dev/null` so disk I/O does not enter the timing.

With `mode: run` the program is measured, not the compiler: the entry is
compiled and linked once, unmeasured, as `<compiler> -std=<std> <args...> -o
<scratch>/bench-run.out bench/<group>/<name>.cpp` (a failed build fails the
entry, with its stderr), and each of the N runs then executes that binary
under the same wall-clock, peak-RSS and memory-cap rules as a compile (the
cap is `--mem-limit`, with no kiln stack reservation). No constexpr or
instantiation report is requested. Optimisation is the entry's business
(`args: -O2`).

A `cargo xtask test` key (`expect`, `diag`, `strict-diags`, `stdout`,
`stdout-file`, `exit`, `run-args`, `run-stdin`, `run-stdin-file`, `env`,
`oracle`, `skip`, any other `xfail`) is an error, as is an unknown key, a missing value,
a repeated `std`/`mode`/`threshold-*`, an `oven` repeated for one location, or
a threshold that is not a non-negative number. A directive
error fails the entry (exit 1) and the entry is not measured.

The command is

```
<compiler> -std=<std> <mode flags> <args...> [-fconstexpr-report=<scratch file>] bench/<group>/<name>.cpp
```

run from the repo root, with stdin and stdout closed (`/dev/null`) and stderr
captured (shown, first 5 lines, if the compile fails). The recorded `flags`
string is everything between the compiler and the file name, except the
report flag, which is a measurement detail rather than a configuration.

## Measurement

Each entry runs N times per compiler; the oracle's runs follow the compiler
under test's for the same entry. A run that fails (non-zero exit, signal, or
the fixed 300 s timeout) fails the entry, and its remaining runs are skipped.

- **Wall clock**: from just before spawn to the return of `wait`, taken by a
  thread blocked in `wait` (no polling quantum). Milliseconds, rounded to µs.
- **Peak RSS**: the child's `ru_maxrss` in KiB, via GNU time
  (`/usr/bin/time -f %M -o <file> -- <command>`); safe Rust cannot call
  `wait4`/`getrusage` without FFI, and the crate forbids `unsafe`. The probe is
  detected once per run (GNU time must accept the format and write a number).
  Without it, memory is not measured and the records say `"rss_source": "none"`.
  The child runs in its own process group, so a timeout kills the wrapper and
  the compiler together.
- **Resource limits**: every run is capped at `--mem-limit` MiB of address
  space (`RLIMIT_AS`), exactly as in `cargo xtask test` (`spec/harness.md`
  "Resource limits"): the `sh -c 'ulimit -v ...; exec "$@"'` wrapper, run
  outside GNU time so the measured process is the compiler; for a kiln
  subject the cap is raised by kiln's stack reservation (the entry's last
  `-fkiln-stack-size=`, else 1024 MiB). A run that runs out of memory under
  the cap fails the entry with `killed (memory limit <cap> MiB address
  space): <how it ended>` and the first lines of its stderr. The cap bounds
  address space, not resident memory, so it does not perturb the peak-RSS
  measurement of a compile within it.
- **Statistics** over the N samples: `min`, `max`, and `median`, where the
  median is taken after dropping one minimum and one maximum sample when
  N >= 3 (the mean of the two middle samples for an even count). Peak RSS is
  the same median over the per-run peaks, and is recorded only if every run
  reported one.
- **Spread** is `(median - min) / min * 100`. A spread over 10% is printed as a
  warning (`noisy`); raise `--runs` or rerun on a quieter host.
- The 1-minute load average (`/proc/loadavg`) is printed and recorded; a load
  above the CPU count is warned about.
- **Constexpr counters** (kiln only, not in `mode: pp` or `mode: run`): every run gets
  `-fconstexpr-report=<scratch>/constexpr-report.jsonl` (removed before the
  run), and the report (`spec/oven.md` "Reporting protocol") is read after it.
  A missing or malformed report, or an `oven:` budget breach, fails the entry.
  The counters are deterministic, so every run must report the same total;
  a difference fails the entry ("nondeterministic constexpr counters"). The
  total's `steps`, `bytes`, `peak_bytes`, `calls`, `allocs`, `peak_arena`,
  `frozen_bytes`, `static_bytes` and `evaluations` are recorded (`wall_us` is not: the wall clock is measured already). clang
  and other compilers get no flag and record no counters.
- **Instantiation counters** (the `inst.*` metrics of
  `spec/sema-templates.md` §11.4; kiln only, entries of the `frontend` group
  only, not in `mode: pp`): every run also gets
  `-fkiln-instantiation-report=json`, and the last `{"total":true,...}` line
  of its stderr (§11.3) is read. A missing or malformed totals row fails the
  entry, and so does a total that differs between runs ("nondeterministic
  instantiation counters"). `count` (`inst.count`), `nodes` and `bytes`
  (`inst.bytes`) are recorded; `self_us` is not. Neither report flag is part
  of the recorded `flags`, so adding them made no baseline advisory. The
  report's timing costs two clock reads per instantiation frame, which is
  inside the wall-clock noise of every frontend entry.

## Regression policy

A measurement is compared only with the baseline of the **same entry and the
same compiler kind** (`kiln`, `clang` or `other`, from the first line of
`--version`), so kiln and clang references coexist in one file.

| Verdict | When | Fails? |
|---|---|---|
| `new` | No baseline for this entry and kind. | no |
| `advisory` | The baseline differs in mode, flags, source hash, compiler version (first `--version` line), or host (arch, CPU count). Deltas are printed but not judged. | no |
| `REGRESSED (time)` | `cur.min > base.median * (1 + t)` and `cur.min - base.median >= 2 ms`. | yes, unless `--record` |
| `REGRESSED (mem)` | `cur.rss > base.rss * (1 + m)`, both sides measured. | yes, unless `--record` |
| `REGRESSED (steps)` | `cur.steps > base.steps * (1 + s)`, both sides have counters. | yes, unless `--record` |
| `REGRESSED (bytes)` | `cur.bytes > base.bytes * (1 + b)` or the same for `peak_bytes`, `peak_arena`, `frozen_bytes` or `static_bytes`, both sides have counters. A key the baseline lacks (`peak_arena` and `frozen_bytes` in a baseline recorded before WP-061b, `static_bytes` before WP-322) is not judged. | yes, unless `--record` |
| `REGRESSED (inst)` | `cur.inst.count > base.inst.count` (exact, whatever the thresholds), or `cur.inst.bytes > base.inst.bytes * (1 + b)`, both sides have instantiation counters. A baseline recorded before WP-086 has none and is not judged on them. | yes, unless `--record` |
| `improved` | `cur.median < base.min * (1 - t)` and `base.min - cur.median >= 2 ms`, or `cur.rss < base.rss * (1 - m)`, or a counter below its baseline by the same rules. | no |
| `ok` | Anything else. | no |

Several regressions are reported together, e.g. `REGRESSED (time, steps)`.
`t`, `m`, `s` and `b` are the effective time, memory, steps and bytes
thresholds as fractions (option, else entry directive, else default). The
counters are exact, so they need no floor and default to tight thresholds:
any extra step is a regression. `inst.count` is exact by policy (§11.4: any
change needs a baseline update in the PR), so `--threshold-steps` does not
widen it; one instantiation fewer is reported as an improvement (re-record).
`calls`, `allocs`, `evaluations` and `inst.nodes` are
recorded for diagnosis but not judged. The
time rule compares the **fastest** current run against the baseline's
**typical** run: host noise only ever adds time, so a load spike moves the
median but rarely every run, while a real slowdown moves them all. The 2 ms
floor absorbs process start-up jitter on very short entries. Measured on the
shared 24-CPU development host at load ~27, repeated clang runs vary by up to
±8% on 1 s entries and ±30% in the median of 0.1 s entries; this rule kept
repeated identical runs green where a median-only or median-and-min rule did
not.

An improvement is a hint to re-record, never a failure.

The libc++ slice is record-only: its entries get no verdict (see "The
libcxx-slice entry").

## Recording

`--record` merges each successfully measured entry into
`<baseline dir>/<group>.json` under its compiler kind, replacing that kind's
previous baseline for the entry and keeping every other entry and kind. Failed
entries are not recorded (their old baseline stays). Only the compiler under
test is recorded, not the `--compare-with` oracle. Regressions are reported
but do not fail a recording run; failures still do.

Baselines are committed. Re-record when an intended change moves the numbers,
when the reference compiler changes, or when the comparison has gone advisory.
A baseline only means something on the host it was recorded on.

A re-record must never absorb a regression nobody explained (WP-798).
Before re-recording an entry that reports `REGRESSED`, classify it:

- **real**: kiln got slower or bigger for no good reason. Fix it; do not re-record.
- **expected**: a change that necessarily costs this. Name the commit.
- **noise or host**: it does not reproduce.

The re-record commit lists the entries and points at the note that
classifies them; `docs/status/bench-regressions-WP-798.md` is the model.

- **Time.** Judge a time regression against an older baseline by an
  interleaved A/B on the current host: a release build of the commit
  that recorded the baseline against the current build, alternating
  `--filter <entry>` runs, comparing the fastest run of each. If the
  baseline commit is as slow today, the regression is host, not kiln.
- **RSS and Oven counters.** These do not depend on load. Bisect them
  with a build per commit and `/usr/bin/time -f %M` or
  `-fconstexpr-report`.

With `--compare-with`, `--record` also **merges** the ratio table (see
"Output") into `docs/status/perf.md`, the same way:

- Every entry the run selected (after `--filter` and `paths`) gets a row
  from this run, replacing its old row or adding one. A failed side shows
  `FAIL`, as in the printed table: the page shows the latest result even
  though the failed entry's baseline is kept.
- Every other row is kept verbatim, including its `Recorded` stamp. Rows
  are ordered by group, then entry name.
- A kept row whose entry is no longer in the corpus (every entry under the
  bench root) is marked **stale**: ``| `<id>` (stale) | ...``. The mark is
  recomputed on every recording, so a row whose entry returns loses it. A
  recording that selected the whole corpus drops the stale rows; it then
  writes exactly the rows of the run, as if the page were new.
- The header (the two compilers with their paths and versions, the host, N,
  and the RSS source) and its date describe the latest recording. Rows
  measured by an earlier recording are identified by their `Recorded`
  stamp: the results-file timestamp of the recording that measured the row,
  which its baselines also carry as `recorded`.
- The geometric-mean line is recomputed over every row of the merged table
  except stale rows, from the millisecond and MB cells as shown (so it can
  be checked from the page alone; a side shown as `FAIL`, `-` or `0.0` gives
  no ratio). The printed ratio table's means use the unrounded medians, so
  the two can differ in the last digit.
- The page is merged only if its table has this run's columns (the same
  compiler-kind labels) and every row parses; the table written before the
  `Recorded` column existed is also read, its rows taking the stamp of its
  "Generated by ... on <date>" line, and so is the table written before the
  instantiations column (before WP-086), its rows showing `-` there. Otherwise (missing, unreadable, other
  compilers, a malformed or repeated row) the page is written from this run
  alone, with a warning on stdout; the rows it held are lost.

- A section headed `## libc++ slice`, between the table and the
  hand-written section, holds the slice's last recording: its header (both
  compilers, the host, N, the RSS source, the load average), the aggregate
  table and the per-test table. A recording that measured slice entries
  rewrites the whole section from them; any other recording keeps it
  verbatim. A recording of the slice alone keeps the main table and its
  header verbatim. The slice's rows never enter the main table, its stale
  marks or its means.
- A section headed `## Measured by hand` (to the end of the page) holds
  measurements no entry makes, such as a libc++ test timed under a debug
  and a release kiln (WP-313). Every recording keeps it verbatim after the
  generated table. The measurements made before 1.0 moved to
  `docs/status/perf-history.md` (WP-1239), which no recording touches.

After writing it the run prints how many rows were recorded, kept (and how
many of those are stale), and dropped.

## Output

Progress lines go to stderr, one per entry and compiler:

```
[3/9] oven/sieve clang: 823.8 ms (min 749.7, spread 9.9%), 117.5 MB
```

The table, results path, ratio table and summary go to stdout:

```
entry                  mode  base ms      ms   time  spread  base MB     MB   mem  status
oven/fib             syntax    720.1   718.2  -0.3%    2.3%     84.3   84.3 -0.0%  ok
...
results: bench/results/2026-10-28T073652.json

bench result: ok. 9 entries: 0 failed, 0 regressed, 1 improved, 0 new, 0 advisory (32.1s)
```

Failed entries are listed with their reason (and stderr head) under
`failures:`. MB are MiB (KiB / 1024).

With `--compare-with`, a ratio table follows: per entry the medians and peak
RSS of both compilers and `test/oracle` ratios (columns named by compiler
kind, or `test`/`oracle` when both are the same kind), then the geometric mean
of the time and memory ratios over the entries both compilers measured. A
failed side shows `FAIL` and is left out of the means. A last column,
`<test> inst`, shows the compiler under test's instantiation totals as
`<count> / <bytes in KiB> KiB` (`-` without them). With `--record`, the
same table (the last column titled `<test> instantiations`), plus a
`Recorded` column, is merged as markdown into
`docs/status/perf.md` (see "Recording"), under a header naming both
compilers and versions, the host, N, and the RSS source of the latest
recording.

## Records

Both formats are JSON written by a dependency-free writer with a fixed key
order (the order below), two-space indentation, and a trailing newline.
Integers are written without a fraction. Readers reject duplicate keys and
unknown `schema` values; they ignore unknown keys. `schema` is 1; bump it on
an incompatible change.

A **measurement** object:

| Key | Type | Meaning |
|---|---|---|
| `mode` | string | `syntax`, `pp`, `emit-llvm`, `obj` or `run` |
| `flags` | string | Flags between the compiler and the file name, space-joined |
| `source_hash` | string | `fnv1a64:<16 hex digits>` of the entry file's bytes (followed by the generated source for a `generate:` entry) |
| `runs` | integer | N |
| `wall_ms_median`, `wall_ms_min`, `wall_ms_max` | number | Milliseconds (µs precision) |
| `spread_pct` | number | Percent, 2 decimals |
| `max_rss_kb` | integer or null | Median peak RSS, KiB; null when not measured |
| `oven` | object, optional | kiln's constexpr report total: `{"steps", "bytes", "peak_bytes", "calls", "allocs", "peak_arena", "frozen_bytes", "static_bytes", "evaluations"}`, integers. `peak_arena` and `frozen_bytes` may be absent (baselines recorded before WP-061b), and `static_bytes` (before WP-322); a missing one is not compared, and is not written back when the entry is not re-recorded. Absent when the compiler reported none. |
| `inst` | object, optional | kiln's instantiation report totals (`frontend` entries): `{"count", "nodes", "bytes"}`, integers. Absent for other entries and compilers, and in baselines recorded before WP-086 (not compared then). |
| `samples_wall_ms`, `samples_rss_kb` | arrays | Every run, in run order (results files only) |
| `compiler` | string | The path or name the compiler was invoked as |
| `compiler_version` | string | First line of `--version` |
| `host` | object | `{"arch": <Rust target arch>, "cpus": <available parallelism>}` |

**Results file** `bench/results/<YYYY-MM-DD>T<HHMMSS>.json` (UTC; `-1`, `-2`,
... is appended if the name exists). Written by every run that measured
anything. The directory is git-ignored: results are local history.

```
{
  "schema": 1,
  "timestamp": "2026-10-28T073652",
  "host": {"arch": "aarch64", "cpus": 24},
  "load_avg_1m": 27.31,            // null where unknown
  "rss_source": "gnu-time",        // or "none"
  "runs": 5,
  "test": <compiler run>,
  "oracle": <compiler run> | null
}
```

A compiler run is `{"compiler", "compiler_kind", "compiler_version",
"entries": {<id>: <outcome>}}`. An outcome is a measurement object with
`"status": "ok"` as its first key, or `{"status": "failed", "error": <text>}`.

**Baseline file** `bench/baselines/<group>.json`:

```
{
  "schema": 1,
  "group": "oven",
  "entries": {
    "fib": {
      "clang": <measurement without samples, plus "recorded": <timestamp>>
    }
  }
}
```

Entry keys are the entry name within the group; the inner keys are compiler
kinds. `recorded` is the timestamp of the results file of the recording run.

## Reflection shapes

`bench/reflect/s1.cpp` .. `s6.cpp` are the six workload shapes of
docs/plans/m6-reflection.md §3.8, and `s7.cpp`/`s8.cpp` the two stress
shapes of the reference-workload profile
(docs/status/perf-reference-workload.md, WP-551), and `s9.cpp` and
`s10.cpp` the reflection-mangling shapes (spec/reflection.md §8.3, WP-501,
WP-879), all written by `cargo xtask gen-reflect`
(`xtask/src/gen_reflect.rs`) and committed at each shape's default N.
They are clean-room: each is a parameterized pattern, not code taken from
any other project.

| Shape | What (N) | Default N | Owner |
|---|---|---|---|
| S1 | N aggregates of 16 members, each serialized by a `template for` over its members (an FNV-1a digest of `name=value;` checked by `static_assert`) | 100 | WP-478 |
| S2 | N enums of 64 enumerators, `enum_to_string` and `string_to_enum` over `enumerators_of` | 50 | WP-478 |
| S3 | one `define_aggregate` of 10N members (built in 10 `define_static_array` chunks), plus N structure-of-arrays types made with `substitute` | 100 | WP-480 |
| S4 | S1 with every third member annotated `[[=key{}]]` and selected by `annotations_of_with_type` | 100 | WP-481 |
| S5 | N `substitute`/`template_arguments_of`/`extract` round trips, in chunks of 100 per evaluation (N a multiple of 100) | 1000 | WP-482 |
| S6 | a namespace of N declarations (functions, classes, variables) scanned by `members_of` in 100 evaluations | 500 | WP-478 |
| S7 | a binding generator: N annotated classes (one in eight unannotated) walked by `members_of` + `annotations_of_with_type`; each class's binding text is built as a `std::string` by a consteval function template, checked per class by `static_assert` on its FNV-1a digest, and printed at run time from a `template for` that also splices the bound members | 40 | WP-551 |
| S8 | the text S7 prints, compiled: N `class_<T>` instantiations of a small clean-room binding library (type casters, `std::tuple` call thunks, `constexpr` signature strings checked by `static_assert`) | 40 | WP-551 |
| S9 | N reflection-keyed instantiations: `f<^^lib::Ti>()` (a function-local static each) and `S<^^lib::Ti>::count` for N namespace-scope types, plus `f<^^W<W<W<lib::Ti>>>>()` for every fourth; no library, `mode: emit-llvm`, so the cost is instantiation, codegen and the mangling of `L Dm <kind> <entity> E` (the default gcc form, WP-909) | 500 | WP-501 |
| S10 | S9's `f<^^X>()` and `S<^^X>::count` for N keys, each X a distinct template-id six levels deep with six arguments per level (`lib::Node<d%4><X(d-1), lib::Tj, int, const lib::Ti*, long, lib::Tj&>`), so every name repeats subtrees; no library, `mode: emit-llvm`; the mangler's load on long nested types (the WP-879 scaling study, spec/reflection.md §8.3 "Cost") | 200 | WP-879 |

Each file starts with a header naming the generator, the shape and N
(`// Generated by `cargo xtask gen-reflect` (…): shape S1, N=100.`),
then `std: c++26`, `args: -freflection` and the §3.8 `oven:` budget:
`peak_arena<=65536` for S1, S2, S4 and S5 (their evaluations do not grow
with N), `peak_bytes` linear in N for S3 (`64*10N + 65536`) and S6
(`16N + 65536`). The counters are deterministic, so the budgets are
gates; wall time and RSS are tracked but never gated in CI. A shape whose
families have not landed is generated with `xfail(kiln): <owner>
(reflection family not landed)` and a "Needs <owner> and ..." header
line; S1–S6 carried it until WP-638, and now say "Exercises <owner> and
...". 

S9 and S10 include no header (`decltype(^^int)` is their info type), so they
carry neither `#include <meta>` nor `xfail(kiln)`; WP-501's numbers
for S9, at N=500 and N=2000 under each `-fkiln-reflection-mangling`
form and against g++-16, and WP-879's depth/width/N scaling study on
S10's pattern, are spec/reflection.md §8.3 "Cost".

S1–S8 compile against GCC 16.1's libstdc++ `<meta>` rather than
libc++ (S7 and S8 since WP-551, S1–S6 since WP-638): they carry no
`xfail(kiln)` marker, and `gen_reflect::kiln_flags` adds the GCC
oracle's `--gcc-toolchain=<prefix>` to every kiln compile of them, in a
bench run and in a scaling sweep (GCC finds its own library). Their budgets are `peak_bytes<=64N + 65536`
for S7 (`bound_classes()` is a vector of N reflections) and
`peak_arena<=65536` for S8, whose only constant evaluations are the
signature strings, flat in N. S7's per-class `static_assert`s keep every
evaluation under the default `-fconstexpr-steps`; a single evaluation over
all N bindings would not be.

```
cargo xtask gen-reflect [--shape <s1..s10>] [--n <N> --out <dir>] [--check]
```

writes every shape (or one) into `bench/reflect/` at its default N;
`--n` needs `--out` (a non-default N never lands in `bench/reflect/`);
`--check` writes nothing and fails (exit 1) when a committed file differs
from what the generator writes. The output is a pure function of shape
and N: same N, same bytes.

## Scaling sweeps

`cargo xtask bench --scaling <shape> [--n N] [--compiler <path>] [--runs R]`
generates the shape at N and 4N into run scratch (never into the repo),
compiles each `R` times with `-std=c++26 -fsyntax-only -freflection`
(`-fconstexpr-report=<file>` too for kiln), and prints a Markdown table
of N, the fastest wall time, the largest peak RSS, and, for kiln, the
report's `steps`, `peak_bytes`, `peak_arena` and `evaluations`, then the
4N/N ratios of wall time and RSS. The gate (§3.8):

- a 4N/N wall-time or peak-RSS ratio above **4.6** fails (exit 1): the
  cost must stay linear, whatever the host's speed;
- for S1, S2, S4, S5 and S8, a `peak_arena` at 4N above the one at N fails:
  their evaluations do not depend on N;
- a failed compile fails, except a kiln failure of an `xfail(kiln)` shape,
  which is reported as `scaling result: xfail (…)` (exit 0); a kiln sweep
  that compiles despite the marker is an XPASS (exit 1).

`--compiler` runs the same sweep under another compiler (GCC 16.1 for the
vs-GCC table in docs/status/perf-history.md "WP-483", which has no counters). Sweeps write no
records and are off the merge queue: wall-time ratios need a quiet host.

## External corpora

`cargo xtask bench --external <dir>` is `cargo xtask test --external`
(spec/harness.md "External corpora") run serially and measured: the same
never-crash verdicts, the same count line, then
`external: compile time S.SSs in total, largest peak RSS M.M MiB`.
Nothing from `<dir>` is written; `-v` adds the failing cases' paths.

## Tests backing this spec

- `xtask/src/util/{json,header,process}.rs`: JSON round-trip, foreign and
  malformed input; directive scanning; measured runs (exit status, stderr,
  RSS, timeout killing the process group, a missing GNU time, the memory cap
  stopping an allocating process).
- `xtask/src/bench/tests.rs` `parses_the_command_line`: `--mem-limit` and
  its default.
- `xtask/src/bench/entry.rs`: directive defaults, every key, rejected headers,
  source hashes; `xfail(kiln)` (reason, repeat); `budget-time` (value,
  repeat).
- `xtask/src/gen_reflect.rs`: same N gives identical bytes, the header,
  the committed files current, every shape's directives parsing (none
  with `xfail(kiln)`; S1–S6 from §3.8 with the "Exercises" header), a
  bad N refused, the FNV-1a digest; `s7_checks_what_s8_compiles` (S8
  binds exactly the classes whose digests S7 checks, and the bare S9
  gets no `kiln_flags`).
- `xtask/src/bench/scaling.rs`: the 4.6x and flat-`peak_arena` gates, the
  table, and a sweep under the fake GCC failing (a compiler without
  `<meta>`) without being masked as xfail.
- `xtask/src/util/oven_report.rs`: report parsing, every rejection, located
  and total budgets.
- `xtask/src/bench/stats.rs`: median after dropping min and max, spread.
- `xtask/src/bench/record.rs`: results and baseline round-trips, timestamps.
- `xtask/src/bench/compare.rs`: every verdict and threshold boundary, the
  absolute floor, advisories, tables, ratios and geometric means.
- `xtask/src/bench/perf_md.rs`: merging `perf.md` rows (kept verbatim,
  replaced, ordered by group then name), the means over the merged table
  from the shown cells, stale rows (marked, out of the means, unmarked when
  the entry returns, dropped by a whole-corpus recording), pages that cannot
  be merged into, the pre-`Recorded` and pre-instantiations layouts, and the
  committed page parsing.
- `xtask/src/bench/slice.rs`: the list format and every rejection, the
  committed list parsing (sorted), explicit selection, ids and entries
  under `--filter`, the shown flags, the distributions (odd and even
  counts, nearest-rank p90, ties for the maximum, zero and non-finite
  ratios), the aggregate over rows with a failed side, the printed report
  and the `perf.md` section, `--help` listing the entry, and
  `refuses_clang_older_than_20` (the clang-20 floor's verdicts and
  message).
- `xtask/src/bench/tests.rs` `slice_refuses_an_oracle_older_than_clang_20`:
  a clang 19 `--compare-with` or `--compiler` refused for the slice before
  any setup, and accepted as the oracle of an ordinary entry.
- `xtask/src/bench/perf_md.rs` `splits_out_the_slice_section`: the page cut
  into table, slice section and hand-written section and put back verbatim,
  and a page gaining the section in its place.
- `xtask/src/libcxx/run.rs` `syntax_args_follow_the_runner`: the slice's
  compile flags are the runner's (base flags, the test's own flags, then
  `-fsyntax-only` or the verify flags), and a `.sh.cpp`, UNSUPPORTED or
  XFAIL test is refused.
- `xtask/src/bench/pch.rs`: the command lines of every case and variant
  (never `/dev/null`, every output a placeholder), the case files present,
  the `-v` notes and the path they give, wall time not judged unless asked
  (a 3x slower run passes, 10% more memory regresses), regions replaced in
  place and added after the last one, the hand-reproduction lines,
  explicit selection (a file under `bench/pch` refused), and the whole
  bench root skipping the group.
- `xtask/src/bench/entry.rs` `rejects_bad_headers`: `mode: pch-build`
  refused as a directive and parsed for records.
- `xtask/src/bench/tests.rs`: command-line parsing, results-name collisions,
  and an end-to-end smoke test with `clang++-19` (skipped when absent) over
  `xtask/fixtures/bench/`: record (with perf.md), compare green, a doctored
  baseline regresses, re-recording replaces it, a broken entry fails and is
  not recorded; and the constexpr counters with `xtask/fixtures/fake-kiln/kiln`
  (see `spec/harness.md`): recorded, unchanged, within an entry's
  `threshold-bytes:`, a one-step regression, a `--threshold-steps` override,
  an improvement, an over-budget entry, and clang recording no counters;
  `time_budget_with_a_fake_kiln` over `xtask/fixtures/bench/budget/`: a
  `budget-time:` breach failing the entry with and without `--record` (not
  recorded), a run inside the budget passing, and clang not held to it;
  the instantiation counters with the same fake kiln over
  `xtask/fixtures/bench/frontend/`: recorded in the baseline and `perf.md`,
  unchanged, a one-instantiation regression despite wide thresholds,
  `inst.bytes` on both sides of 2%, an improvement, a pre-WP-086 baseline
  not judged, a missing totals row failing the entry, and clang recording
  none;
  and `perf.md` across recordings with clang as both compilers: written
  fresh when absent, a filtered recording keeping another group's row
  verbatim, a re-recording replacing a doctored row, a stale row marked and
  out of the means, a whole-corpus recording dropping it, and an unreadable
  page rewritten.
