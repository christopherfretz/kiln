# Bench: `cargo xtask bench`

Status: implemented (WP-004; constexpr counters and per-entry thresholds
WP-006; `perf.md` merging WP-006b). Code: `xtask/src/bench/`, with shared pieces in
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
| `--compiler <path>` | Compiler to measure. Default: `cargo build --release -p kiln_driver`, then `<target>/release/kiln` (a debug kiln says nothing about performance). |
| `--baseline <dir>` | Baseline directory. Default `bench/baselines`. Relative paths are taken from the repo root. |
| `--record` | Write the measurements as the new baselines (see "Recording"). With `--compare-with`, also merge the ratio table into `docs/status/perf.md`. |
| `--compare-with <path>` | Also measure this compiler (normally `clang++-19`) on the same entries and print the ratio table. |
| `--filter <substr>` | Keep only entries whose id (`<group>/<name>`) contains `<substr>`. |
| `--runs <N>` | Runs per entry, N >= 1. Default 5. |
| `--threshold-time <PCT>` | Allowed wall-clock regression, percent, >= 0. Default 5. |
| `--threshold-mem <PCT>` | Allowed peak-RSS regression, percent, >= 0. Default 2. |
| `--threshold-steps <PCT>` | Allowed constexpr `steps` regression, percent, >= 0. Default 0. |
| `--threshold-bytes <PCT>` | Allowed constexpr `bytes`/`peak_bytes`/`peak_arena`/`frozen_bytes` regression, percent, >= 0. Default 2. |

A `--threshold-*` option overrides the same entry's `threshold-*:` directive
(below), which overrides the default. Options apply to every selected entry.
| `-v, --verbose` | Print each compiler command. |
| `-h, --help` | Usage. |

`--flag=value` is accepted for every option that takes a value. `paths` are
files or directories under the bench root (default: all of `bench/`); a
relative path that does not exist from the current directory is taken from
the repo root.

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
generated inputs. An entry may include itself (by its literal file name, not
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
spec/sema-overload.md §11).

## Directives

The header is the `//@ key: value` lines before the first line of code, in
the same syntax as `cargo xtask test` (`spec/harness.md` "Directives"). The
scanner is shared; the keys are not.

| Key | Meaning |
|---|---|
| `std: <std>` | Language standard, one of the C++ values accepted by `cargo xtask test`. Default `c++26`. |
| `args: <flags>` | Extra compiler flags, whitespace-split; may repeat (concatenated in order). |
| `mode: syntax\|pp\|emit-llvm` | What the compiler does (below). Default `syntax`. |
| `oven: [<line>[:<col>]] <counter><=N ...` | A constexpr budget, as in `cargo xtask test` (`spec/harness.md` "kiln-specific tests"); may repeat, once per location. Checked on every kiln run; a breach fails the entry. An error with `mode: pp`. |
| `threshold-time: <PCT>` | This entry's allowed wall-clock regression (default 5). |
| `threshold-mem: <PCT>` | This entry's allowed peak-RSS regression (default 2). |
| `threshold-steps: <PCT>` | This entry's allowed constexpr `steps` regression (default 0). |
| `threshold-bytes: <PCT>` | This entry's allowed constexpr `bytes`/`peak_bytes`/`peak_arena`/`frozen_bytes` regression (default 2). |

| Mode | Flags |
|---|---|
| `syntax` | `-fsyntax-only` |
| `pp` | `-E -o /dev/null` |
| `emit-llvm` | `-S -emit-llvm -o /dev/null` |

Output goes to `/dev/null` so disk I/O does not enter the timing.

A `cargo xtask test` key (`expect`, `diag`, `strict-diags`, `stdout`,
`stdout-file`, `exit`, `run-args`, `run-stdin`, `run-stdin-file`, `env`,
`oracle`, `skip`, `xfail`) is an error, as is an unknown key, a missing value,
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
- **Statistics** over the N samples: `min`, `max`, and `median`, where the
  median is taken after dropping one minimum and one maximum sample when
  N >= 3 (the mean of the two middle samples for an even count). Peak RSS is
  the same median over the per-run peaks, and is recorded only if every run
  reported one.
- **Spread** is `(median - min) / min * 100`. A spread over 10% is printed as a
  warning (`noisy`); raise `--runs` or rerun on a quieter host.
- The 1-minute load average (`/proc/loadavg`) is printed and recorded; a load
  above the CPU count is warned about.
- **Constexpr counters** (kiln only, not in `mode: pp`): every run gets
  `-fconstexpr-report=<scratch>/constexpr-report.jsonl` (removed before the
  run), and the report (`spec/oven.md` "Reporting protocol") is read after it.
  A missing or malformed report, or an `oven:` budget breach, fails the entry.
  The counters are deterministic, so every run must report the same total;
  a difference fails the entry ("nondeterministic constexpr counters"). The
  total's `steps`, `bytes`, `peak_bytes`, `calls`, `allocs`, `peak_arena`,
  `frozen_bytes` and `evaluations` are recorded (`wall_us` is not: the wall clock is measured already). clang
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
| `REGRESSED (bytes)` | `cur.bytes > base.bytes * (1 + b)` or the same for `peak_bytes`, `peak_arena` or `frozen_bytes`, both sides have counters. A key the baseline lacks (`peak_arena` and `frozen_bytes` in a baseline recorded before WP-061b) is not judged. | yes, unless `--record` |
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
| `mode` | string | `syntax`, `pp` or `emit-llvm` |
| `flags` | string | Flags between the compiler and the file name, space-joined |
| `source_hash` | string | `fnv1a64:<16 hex digits>` of the entry file's bytes |
| `runs` | integer | N |
| `wall_ms_median`, `wall_ms_min`, `wall_ms_max` | number | Milliseconds (µs precision) |
| `spread_pct` | number | Percent, 2 decimals |
| `max_rss_kb` | integer or null | Median peak RSS, KiB; null when not measured |
| `oven` | object, optional | kiln's constexpr report total: `{"steps", "bytes", "peak_bytes", "calls", "allocs", "peak_arena", "frozen_bytes", "evaluations"}`, integers. `peak_arena` and `frozen_bytes` may be absent (baselines recorded before WP-061b); a missing one is not compared, and is not written back when the entry is not re-recorded. Absent when the compiler reported none. |
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

## Tests backing this spec

- `xtask/src/util/{json,header,process}.rs`: JSON round-trip, foreign and
  malformed input; directive scanning; measured runs (exit status, stderr,
  RSS, timeout killing the process group, a missing GNU time).
- `xtask/src/bench/entry.rs`: directive defaults, every key, rejected headers,
  source hashes.
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
- `xtask/src/bench/tests.rs`: command-line parsing, results-name collisions,
  and an end-to-end smoke test with `clang++-19` (skipped when absent) over
  `xtask/fixtures/bench/`: record (with perf.md), compare green, a doctored
  baseline regresses, re-recording replaces it, a broken entry fails and is
  not recorded; and the constexpr counters with `xtask/fixtures/fake-kiln/kiln`
  (see `spec/harness.md`): recorded, unchanged, within an entry's
  `threshold-bytes:`, a one-step regression, a `--threshold-steps` override,
  an improvement, an over-budget entry, and clang recording no counters;
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
