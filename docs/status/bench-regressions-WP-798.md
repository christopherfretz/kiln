# Bench regressions on main b1a2c66a7 (WP-798)

`cargo xtask bench` on main b1a2c66a7 reported 20 regressed entries
against `bench/baselines/*.json`. This page gives each one a class and,
where there is one, a first-bad commit. The rule for doing this is now in
spec/bench.md "Recording".

**Host.** The shared aarch64 host has 24 CPUs. Load average was 12–14 the
whole time.

**Build.** Every binary is `cargo build --release -p kiln_driver`, the
bench's own profile. The commit under test is checked out in a detached
worktree under `.worktrees/wp-798/.bisect/`.

**How each number was measured.** `<kiln>` is that binary and
`-resource-dir` is its own `resource/`.

| Key | Command | What it measures |
|---|---|---|
| bench | `cargo xtask bench --compiler <kiln> --filter <entry> --runs 3` | The fastest run is the "min" figure. |
| A/B | the bench command, alternating baseline-commit build and head build | Rounds as given in the row; the fastest run of each side. |
| rss | `/usr/bin/time -f %M <kiln> -std=c++26 -fsyntax-only <file>` | Peak RSS in KiB. The median of 5 where marked; otherwise one run, which is noisy to about ±300 KiB. |
| static | `<kiln> -std=c++26 -fsyntax-only -fconstexpr-report=r.json <file>` | `total.static_bytes`, read from the report. |
| cfg | `<kiln> -std=c++26 -fsyntax-only -fkiln-cfg-stats <file>` | Graph elements and bytes for each body. |
| thunks | `<kiln> -std=c++26 -fsyntax-only -fkiln-memory-report <file>` | The `oven.thunks` and `oven.code` lines. |
| stack | the smallest `-fkiln-stack-size=<N>k` that compiles `<file>`, binary-searched in 64 KiB steps | Stack depth used, in KiB. |
| inst | `<kiln> ... -fkiln-instantiation-report[=all] <file>` | The `total:` line. |

**Commits.**

- e46fe36ee recorded the baselines for `overload/*`, `simd-generate`,
  `templates_deep`, `nested-template-ids` and `virtual-lattice*` on 11-03.
- f783ec5c7 recorded `oven/*` (WP-662, 11-06).
- f57783a9c recorded `constant-table-emit` (WP-665).
- 4f9a3de97 recorded `registration-wide-obj` (WP-641).
- e14b98bbc recorded `accessor-wide-emit` (WP-633).

**Head** is this branch, 861aa4d18 or later. Its full bench log is
`cargo xtask bench --compiler <head>`, run while bisect builds were going,
so its times are not to be trusted.

## Summary

- **Real, fixed (3 commits).**
  - `overload/arrow-chain`: RSS 138.3 → 78.5 MB, time 827 → 538 ms.
  - Part of `simd-generate` (static), `less-10k` and `ctors`.
  - Root causes:
    - WP-458's `-Winteger-overflow` scan left one compiled Oven thunk behind
      for every operand it failed to fold.
    - WP-509 built a control-flow graph for every value-returning body only to
      answer -Wreturn-type.
- **Expected.**
  - The Rust toolchain pin to 1.95 (WP-614) made frames on the deep
    recursion paths about 1.3 KiB per level bigger.
  - Code growth added about 0.8 MB to every compile: an empty TU went from
    6.5 to 7.3 MB as `.text` grew from 10.1 to 12.4 MB over 302 commits.
  - The overflow scan (WP-458) evaluates operands the way clang's
    `EvaluateForOverflow` does, and touches 16–512 B of static storage.
- **Noise or host.** Every time regression. A build of the commit that
  recorded the baseline is as slow today, in an interleaved A/B, as head.

## Entries

The columns are:

- **Baseline → main**: the bench medians, as reported against b1a2c66a7.
- **Head**: this branch.
- **First-bad**: confirmed by building both the commit and its first parent.

| Entry | Kind | Baseline → main | Head | First-bad | Class |
|---|---|---|---|---|---|
| overload/arrow-chain | time, mem, bytes | 674 ms, 77.3 MB, static 0 → 827 ms, 138.3 MB, static 8 | 538 ms, 78.5 MB, static 0 | 96dfea53f (#192 WP-458): rss 79.8 → 116.9 MB; c9129d450 (#218 WP-509): rss 116.9 → 141.2 MB | **real, fixed** (see "Fixes") |
| frontend/simd-generate | bytes | static 155,544 → 160,536 | 155,544 | c9129d450 (#218 WP-509): static 155,544 → 160,536 | **real, fixed** by 861aa4d18. RSS improved (73.0 → 60.4 MB). |
| overload/less-10k | mem | 17.3 → 19.7 MB | 18.6 MB | c9129d450 (#218 WP-509): rss 18.6 → 20.0 MB | **real, fixed** (rss at head 18.8 MB). The remaining +1.3 MB over the baseline is **expected**, code growth: the empty TU went 6.53 → 7.30 MB. |
| overload/ctors | mem, bytes | 12.2 MB, static 512 → 13.7 MB, 1024 | 13.3 MB, 1024 | static: 96dfea53f (#192 WP-458), 512 → 1024. rss: c9129d450 +0.8 MB. | rss: WP-509's part **fixed** (14.40 → 14.02 MB); the rest is **expected**, code growth. static: **expected**, WP-458 (`-Wno-integer-overflow` gives 512). |
| overload/calls | mem | 13.8 → 15.2 MB | 14.4 MB | none (spread) | **expected**, code growth. rss e46fe36ee 14.40 → head 15.17 MB, which matches the empty TU's +0.76 MB. |
| frontend/nested-template-ids | mem | 18.0 → 20.1 MB | 20.1 MB | 7b9507098 (#276 WP-614): stack 6848 → 7936 KiB, rss 19.43 → 20.59 MB | **expected**, toolchain 1.99.1 → 1.95.0 (operator constraint, WP-614). Recursion 2,000 levels deep: about 0.5 KiB more per level. Plus code growth. |
| frontend/templates_deep | mem | 37.7 → 40.5 MB | 40.2 MB | 7b9507098 (#276 WP-614): stack 9472 → 10816 KiB, rss 39.43 → 40.90 MB | **expected**, WP-614 as above (1,000 instantiation levels). Plus code growth. |
| frontend/virtual-dtor-chain | mem | 20.5 → 21.8 MB | 21.8 MB | none (spread): median-of-5 rss 96dfea53f 20.4 → d4acbce85 21.6 → head 22.0 MB | **expected/noise**. Code growth explains +0.76 MB. The bisect stopped at c33a79ca8 (#191 WP-454, 21296 KiB), but a re-measure of it against its parent 47a80ba99 did not reproduce (20,740 vs 20,832 KiB). Not stack: it needs the minimum, 4160 KiB, everywhere. |
| frontend/virtual-lattice | mem, bytes | 9.4 MB, static 0 → 10.9 MB, 16 | 10.7 MB, 16 | static: 96dfea53f (#192 WP-458) | static: **expected**, WP-458. The scan folds `r.f() + r.g()`, and `-Wno-integer-overflow` gives 0. rss: **expected**, code growth (e46fe36ee 9.97 → head 10.95 MB). |
| frontend/virtual-lattice-emit | mem, bytes | 23.2 MB, static 0 → 24.9 MB, 16 | 25.8 MB, 16 | static: 96dfea53f (#192 WP-458) | static: **expected**, WP-458. rss: **noise**, plus code growth. Five runs of one binary give 23.7–25.9 MB (96dfea53f) and 24.9–26.7 MB (head): the peak is bimodal. |
| frontend/accessor-wide-emit | inst | 141,981 → 141,987 | 141,987 | 2df9a8322 (#327 WP-709): inst 141,981 → 141,987 (parent f415f77ee) | **expected**: WP-709 ([temp.spec.partial.general]/7) also converts a declarator qualifier's arguments with `check_template_args` to find a partial specialization, which substitutes default template arguments: +2 `__make_signed_selector`, +4 `basic_string` default-argument substitutions (`-fkiln-instantiation-report=all` diff). Retained inst KiB −0.4%. |
| frontend/constant-table-emit | time | 498.5 → 550.1 ms | 578 ms (loaded) | – | **noise/host**. A/B, 3 rounds: f57783a9c 554.5 ms, head 550.4 ms. |
| frontend/registration-wide-obj | time | 6987.5 → 7953.7 ms | 8609 ms (loaded) | – | **noise/host**. A/B, 3 rounds: 4f9a3de97 7836.1 ms, head 7754.0 ms. |
| oven/churn-string | time | 398.7 → 430.1 ms | – | – | **noise/host**. A/B, 3 rounds: f783ec5c7 418.6 ms, head 424.1 ms. |
| oven/churn-vector | time | 466.6 → 509.2 ms | – | – | **noise/host**. A/B: 494.2 vs 488.8 ms. |
| oven/sort | time | 177.4 → 197.4 ms | – | – | **noise/host**. A/B: 184.3 vs 191.7 ms (+4%, under the 5% threshold). |
| oven/table-nohdr | time | 45.5 → 50.1 ms | – | – | **noise/host**. A/B: 45.1 vs 42.7 ms. |
| oven/vector-alloc | time | 367.8 → 410.7 ms | – | – | **noise/host**. A/B: 412.4 vs 401.1 ms. |
| oven/wide-struct-store | time | 126.9 → 147.1 ms | – | – | **noise/host**. A/B: 138.0 vs 132.1 ms. |
| oven/builtins-memcpy | mem | 9.9 → 10.1 MB (+2.0%) | 9.9 MB (ok) | – | **noise**. It sits exactly at the 2% threshold, and the head run passes. |

**Oven counters.** The Oven's `steps` and `bytes` are unchanged on every
`oven/*` entry.

**Brief entries that did not regress here.** The brief named
`oven/virtual-dispatch` and `oven/wide-struct-read`. Their A/B against
f783ec5c7:

- `virtual-dispatch`: 17.8 vs 18.4 ms.
- `wide-struct-read`: 296.1 vs 294.3 ms.

Both are host noise.

## Fixes

Each fix is its own commit. Numbers are the fastest of 3 for time and the
peak for RSS, on `bench/overload/arrow-chain.cpp` with `-fsyntax-only`.

| Commit | Change | Measured on arrow-chain |
|---|---|---|
| 6ba4c2a0d | `kiln_oven`: a failed trial fold gives its thunk's code back (`crates/kiln_oven/src/reclaim.rs`). | RSS 139.3 → 103.3 MB. thunks 6,002 → 3; `oven.code` 4.86M → 829 words. Time unchanged. |
| 7d7525f0e | `kiln_sema`: the overflow scan does not fold an operand that calls a non-constexpr member function. The Oven refuses that call, so the fold cannot succeed. (`crates/kiln_sema/src/int_overflow.rs` `overflow_fold_fails`) | 792 → 580 ms. |
| 861aa4d18 | `kiln_sema`: -Wreturn-type answers `Never` for a body ending in `return` without building its graph (`crates/kiln_sema/src/stmt.rs` `falls_off`). | RSS 102.8 → 80.5 MB, 582 → 521 ms. The graph of `use` was 3,012,006 elements, 24 MB. |

**Where arrow-chain's memory went.** `use` is 12,000 member accesses
through an `operator->` chain. Its RSS grew in two steps:

1. **WP-458**, 96dfea53f: 79.8 → 116.9 MB.
   - The new `-Winteger-overflow` scan asked the Oven to fold every signed
     `+` operand.
   - Each fold compiled a thunk, which failed at the first non-constexpr
     `operator->` and stayed cached forever: 6,002 thunks and 4.86M code
     words.
2. **WP-509**, c9129d450: 116.9 → 141.2 MB.
   - `flush_runtime_diags` built the body's control-flow graph for
     -Wreturn-type: 8 bytes per expression.

**Remaining cost.** Without `-Winteger-overflow`, arrow-chain takes 452 ms,
so the scan still costs about 70 ms. That is its walk plus the folds of
operands that might succeed.

## Reproduction log

Every finding above can be re-measured with the commands below. The
numbers in this section come from one sweep, on 11-07 at load average
14 on 24 CPUs.

**Build a commit.**

```sh
git worktree add --detach .bisect/b <sha>
cd .bisect/b && source scripts/env.sh && cargo build --release -p kiln_driver
```

Then copy `$CARGO_TARGET_DIR/release/kiln` and `resource/` aside, and
run `git worktree remove .bisect/b`. `fix3` is 861aa4d18, the last fix.

**Measure an entry.**

```sh
/usr/bin/time -f %M kiln -resource-dir <its resource> <flags> bench/<entry>.cpp
```

Each entry was run 5 times. The figures are the fastest wall time and the
median peak RSS.

`<flags>` are the bench's own flags for the entry, as its baseline JSON
records them:

- `syntax` mode: `-std=c++26 -fsyntax-only`, plus the entry's `//@ args`
  (the `oven/*` entries add `-fconstexpr-steps=...`).
- `emit-llvm` mode: `-std=c++26 -S -emit-llvm -o /dev/null`.
- `obj` mode: `-std=c++26 -c -o /dev/null -O2`.

Each entry below gives wall time and peak RSS for each commit, oldest
first. The first-bad commit and its parent are named in the Entries
table.

- `overload/arrow-chain`: dfd0c4b28 640 ms / 79.7 MB, 96dfea53f 963 ms /
  116.2 MB, d4acbce85 1133 ms / 116.6 MB, c9129d450 1076 ms / 141.9 MB,
  fix3 561 ms / 81.8 MB.
- `frontend/simd-generate`: d4acbce85 603 ms / 66.4 MB, c9129d450 742 ms /
  67.5 MB, fix3 482 ms / 59.4 MB.
- `overload/less-10k`: e46fe36ee 2040 ms / 18.1 MB, d4acbce85 2148 ms /
  18.5 MB, c9129d450 2088 ms / 20.0 MB, fix3 1748 ms / 18.9 MB.
- `overload/ctors`: e46fe36ee 579 ms / 12.5 MB, dfd0c4b28 584 ms /
  12.1 MB, 96dfea53f 585 ms / 13.1 MB, d4acbce85 588 ms / 13.6 MB,
  c9129d450 602 ms / 14.2 MB, fix3 546 ms / 14.1 MB.
- `overload/calls`: e46fe36ee 1744 ms / 14.3 MB, fix3 1043 ms / 15.0 MB.
- `frontend/nested-template-ids`: e46fe36ee 569 ms / 18.6 MB, 1fe2ec39d
  602 ms / 19.2 MB, 7b9507098 621 ms / 20.5 MB, fix3 550 ms / 20.7 MB.
- `frontend/templates_deep`: e46fe36ee 60.1 ms / 38.6 MB, 1fe2ec39d
  62.0 ms / 39.4 MB, 7b9507098 60.8 ms / 40.5 MB, fix3 62.9 ms /
  41.0 MB.
- `frontend/virtual-dtor-chain`: 47a80ba99 3295 ms / 21.0 MB, c33a79ca8
  3247 ms / 21.3 MB, fix3 200 ms / 22.2 MB.
- `frontend/virtual-lattice`: e46fe36ee 16.7 ms / 9.6 MB, dfd0c4b28
  16.1 ms / 9.7 MB, 96dfea53f 15.1 ms / 9.9 MB, fix3 15.6 ms / 11.0 MB.
- `frontend/virtual-lattice-emit`: e46fe36ee 143 ms / 23.8 MB, dfd0c4b28
  142 ms / 25.0 MB, 96dfea53f 142 ms / 23.8 MB, fix3 119 ms / 25.9 MB.
  The peak is bimodal.
- `frontend/accessor-wide-emit`: e14b98bbc 1988 ms / 139.3 MB, f415f77ee
  1737 ms / 139.4 MB, 2df9a8322 1770 ms / 139.4 MB, fix3 1670 ms /
  139.2 MB.
- `frontend/constant-table-emit`: f57783a9c 553 ms / 124.2 MB, fix3
  550 ms / 124.1 MB.
- `frontend/registration-wide-obj`: 4f9a3de97 8610 ms / 262.1 MB, fix3
  8475 ms / 263.1 MB.
- `oven/*`, f783ec5c7 then fix3:
  - `builtins-memcpy`: 24.4 ms / 10.4 MB, then 13.2 ms / 10.1 MB.
  - `churn-string`: 563 ms / 11.0 MB, then 434 ms / 10.8 MB.
  - `churn-vector`: 506 ms / 11.0 MB, then 491 ms / 10.8 MB.
  - `sort`: 199 ms / 24.9 MB, then 199 ms / 24.9 MB.
  - `table-nohdr`: 44.8 ms / 10.8 MB, then 42.9 ms / 10.6 MB.
  - `vector-alloc`: 427 ms / 36.6 MB, then 425 ms / 36.2 MB.
  - `wide-struct-store`: 141 ms / 31.2 MB, then 138 ms / 28.9 MB.

These wall times include process start-up and were taken under load.
They back the A/B verdicts: no `oven/*` or time-only entry is measurably
slower at fix3 than at the commit that recorded its baseline. The worst
is `sort`, at +0.5 ms.

## Re-recorded

After the fixes and a merge of main, the entries below were re-recorded.
The command was `cargo xtask bench --record <their bench/*.cpp paths>`,
using the release kiln++ of the merged head. The results are in
`bench/results/2026-11-07T211934.json`. Each group's re-record commit
lists its entries. No `budget-time` line was changed.

- `overload/`: calls, ctors, less-10k.
- `frontend/`: accessor-wide-emit, nested-template-ids, templates_deep,
  virtual-dtor-chain, virtual-lattice, virtual-lattice-emit.

**`templates_deep` keeps its old time fields.** The recording ran at load
14 on 24 CPUs and measured 74.6 ms against 63.5 ms. Under the A/B rule
that is the host, not kiln. Taking the new figure would loosen the time
gate by 17%.

**Not re-recorded.**

- The time-only noise/host entries: `constant-table-emit`,
  `registration-wide-obj` and every `oven/*` entry.
  - The A/B shows the commit that recorded each baseline is as slow on
    today's host as head.
  - So the old baselines are still valid, and they are the tighter ones.
  - Recording them at load 14 would have absorbed the load: for example,
    `builtins-memcpy` measured 24.0 ms against 13.9 ms.
- `builtins-memcpy`: its memory passes at head (9.7 MB, −2.3%).
- `overload/arrow-chain` and `frontend/simd-generate`: with the fixes
  they are within threshold or improved.

**Within threshold, not flagged.** `accessor-wide-emit`'s `static_bytes`
went 11,144 → 11,152 (+0.07%).

- It is 11,144 at both f415f77ee and 2df9a8322, and 11,152 at head.
- It does not come from the overflow scan: `-Wno-integer-overflow` gives
  the same figure.
- The re-record takes it in.

## Prevention (proposal, not implemented)

Nothing in CI runs the bench, and the baselines are host-specific. That is
how WP-458 and WP-509 landed a 61 MB regression unnoticed. Two cheap
checks would have caught it.

1. **A counters-only bench step in `cargo xtask ci`.**
   - Run the debug kiln that CI already builds over the `overload/` and
     `frontend/` syntax-only entries.
   - Pass `--threshold-time 100000 --threshold-mem 100000`, so only the
     exact counters are judged: `steps`, `bytes`, `static_bytes` and
     `inst.count`. These do not depend on host, load or build profile.
   - This would have flagged WP-458 (arrow-chain static 0 → 8, ctors
     512 → 1024) and WP-509 (simd-generate static).
   - Estimated cost: about 1 min of debug-kiln time.
   - Not implemented, for three reasons:
     - The x86_64 CI lane has no aarch64-recorded counters to compare with.
     - Some baselines would need re-recording per target.
     - Validating it needs CI runs this WP cannot make.
2. **A deterministic heap figure in the bench JSON.**
   - Record the high-water of the tracked allocations that
     `-fkiln-memory-report` already counts, and add the control-flow graph
     to them.
   - Judge it like `bytes`, exactly.
   - RSS is host-noisy by ±300 KiB on small entries; a tracked-heap figure
     is not. Both arrow-chain steps (Oven code tables, the graph) would show
     as exact counter regressions at PR time.
   - Estimated at 150–250 lines across `kiln_sema`, `kiln_driver` and
     `xtask/src/bench`.
   - Left as a follow-up.
