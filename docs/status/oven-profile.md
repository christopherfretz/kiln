# Oven per-step overhead on standard-library plumbing (WP-1864)

Where the Oven's time went on a reduction of real code that compares many
`std::string_view`s built from `std::string`s (libstdc++ 14, C++26), what
WP-1864 changed, and what is left. Measured on 2026-11-21 on a shared
aarch64 host (24 CPUs, 1-minute load 10-12), release builds, medians of
3-6 interleaved runs; base is main at 22256170f. It is a record of those
measurements and is not regenerated. The contracts are spec/oven.md
("Cost split", "Library fast paths") and spec/driver.md ("Constexpr
profile").

## The reduction

N names `field_0` .. `field_<N-1>` pushed into a `std::vector<std::string>`
(each fits the small buffer), viewed as a `std::vector<std::string_view>`,
then every pair compared with `==`, all in one `consteval` call:
`bench/oven/string-view-pairs.cpp` at N=1000. Each comparison is
`operator==` → `size()` and `char_traits<char>::compare` →
`std::__is_constant_evaluated()` → the constant-evaluation loop of
`lt`/`eq` calls, over the bytes of a `union { char buf[16]; size_t cap; }`.

| N | | wall | steps | calls | memo hits |
|---|---|---|---|---|---|
| 1000 | before | 2.26-2.33 s | 41,617,762 | 13,247,035 | 429,552 |
| 1000 | after | 0.55 s | 14,950,089 | 3,529,248 | 0 |
| 2000 | before | 5.82 s | 103,942,901 | 33,851,406 | 952,174 |
| 2000 | after | 1.31 s | 44,687,240 | 12,257,997 | 0 |
| 4300 | before | 32.22 s | 616,807,796 | 198,061,033 | 5,968,148 |
| 4300 | after | 5.96 s | 235,634,564 | 59,107,825 | 0 |

The table is for the same program with a 10-character prefix (the bench
entry's 6-character one is in "Bench"); flags `-std=c++26 -fsyntax-only
-fconstexpr-steps=2000000000 -fconstexpr-stats`. N=4300 is 5.4x faster;
the peak RSS is unchanged (35 MB). The memo hits were calls the fast
paths now make without a call.

## The cost split

`-fconstexpr-profile-split` (new here) samples the VM's phase about once
per millisecond, in a kiln built with the `kiln_oven/split` feature
(the numbers below were taken with its first design, the same stores in
a second instantiation of the VM loop; see "The split's own cost").
N=1000:

| | dispatch | checks | memory | memo | bridge | frames | VM time |
|---|---|---|---|---|---|---|---|
| before (main + the split) | 15-17% | 23-27% | 51-52% | 1% | 0% | 6.5% | ~2150 ms |
| path cache + union memo fixed | 34-36% | 12% | 41% | 0.5% | 0% | 10-12% | ~1300 ms |
| after (all of WP-1864) | 20% | 4-5% | 60-63% | 0% | 0% | 11-14% | ~365 ms |

The final `split` build gives the same picture after the work (N=1000,
two runs): dispatch 18%, checks 3-6%, memory 62-66%, frames 13-14%,
~380 ms; `string-view-find-if`: dispatch 17-21%, checks 6-8%, memory
55-60%, frames 16-17%.

The split before the work pointed at the memory model and the checks, not
at dispatch or frames, and callgrind (exclusive instructions, N=150 minus
N=2) said why: 36% of the evaluation was `PtrAdd`'s walk of the type
path, and 9% the union-member checks of loads.

## What changed

- **The path cache missed every array inside a union** (`ty/path.rs`):
  the whole-array extent of std::string's `buf` was never cached, because
  the uniqueness test refused any union with two members over the
  address (`buf` and `cap`), although only `buf` can hold a `char`.
  `may_hold` now asks whether a member can contain the element type at
  all. Every `PtrAdd` into the small buffer walked the type path before;
  it is now a cache hit. Same steps, 2.3 s to 1.47 s.
- **The union path memo thrashed** (`vm/umemo.rs`): one slot per
  (allocation, type) meant comparing two strings evicted each other's
  entry on every access. The slot is now two-way.
- **Wrappers of the evaluation context cost nothing** (`compile/fast.rs`):
  `std::__is_constant_evaluated()`, `std::is_constant_evaluated()` (both
  `if consteval` in libstdc++ C++23 and up) and libc++'s
  `std::__libcpp_is_constant_evaluated()` compile to the context read, no
  call. Charged 0 steps, as clang charges `std::move`.
- **`std::char_traits<C>::compare`, `length` and `find` run natively**
  (`vm/fast.rs`) for the five character types, recognized once per call
  site at KBC compile time by the declaration (a member of
  `std::char_traits`, its name and parameter types). A run in one array
  is checked once; anything else (a read past the end, a dead object, a
  budget the call would cross) declines, and the library's loop runs and
  diagnoses as before. Charged one step per element examined.

Steps fell because the fast paths charge fewer than the library code
they replace; user code is counted as clang counts it
(`expr.const/budget/fast-paths*.cpp`).

`bytes` (391 MB at N=4300 from 15 heap allocations, 0.4 MB peak live)
was not a bug: it is every byte the Oven allocated, frames included, and
every comparison copies two `string_view`s (by value) into a frame. It
is now documented that way (spec/oven.md "Profile counters").

## Bench

`cargo xtask bench bench/oven`, kiln before (main) and after, same host,
alternating whole runs (before, after, before, after); times are the
fastest of the 10 runs of each. On this host the time of an unchanged
entry moves by up to ±8% between runs: callgrind counts the same
instructions for `virtual-dispatch` (+7.3% here), `sieve` (+7.7%) and
`builtins-memcpy` (+5.6%) before and after, within 0.1%. Steps fall
where a fast path applies (`string` is libc++'s, whose
`__libcpp_is_constant_evaluated` is no longer a call).

| entry | before ms | after ms | time | before MB | after MB | before steps | after steps |
|---|---|---|---|---|---|---|---|
| builtins-memcpy | 25.2 | 26.6 | +5.6% | 14.9 | 15.0 | 10,215 | 10,215 |
| builtins-popcount | 73.7 | 74.4 | +0.9% | 14.8 | 14.9 | 1,600,010 | 1,600,010 |
| builtins-strlen | 24.9 | 24.3 | -2.4% | 15.0 | 14.9 | 14,115 | 14,115 |
| churn-list | 37.2 | 36.2 | -2.7% | 15.2 | 14.9 | 610,005 | 610,005 |
| churn-mixed | 23.7 | 24.3 | +2.5% | 15.0 | 14.9 | 100,102 | 100,102 |
| churn-string | 461.5 | 457.9 | -0.8% | 15.6 | 15.3 | 6,002,085 | 6,002,085 |
| churn-tree | 42.9 | 41.5 | -3.3% | 15.5 | 15.4 | 1,179,557 | 1,179,557 |
| churn-vector | 530.2 | 531.2 | +0.2% | 15.6 | 15.4 | 8,398,277 | 8,398,277 |
| class-churn | 170.1 | 169.9 | -0.1% | 17.4 | 17.1 | 2,960,095 | 2,960,095 |
| fib | 20.8 | 21.7 | +4.3% | 14.8 | 14.5 | 786,024 | 786,024 |
| inplace-alloc | 181.5 | 182.2 | +0.4% | 26.1 | 25.8 | 2,500,540 | 2,500,536 |
| inplace-union | 187.7 | 189.0 | +0.7% | 25.9 | 25.8 | 2,500,518 | 2,500,518 |
| sieve | 92.2 | 99.3 | +7.7% | 22.2 | 21.9 | 684,419 | 684,419 |
| sieve-nohdr | 28.0 | 28.8 | +2.9% | 14.9 | 14.7 | 253,992 | 253,992 |
| soa | 73.2 | 73.9 | +1.0% | 15.4 | 15.2 | 1,745,460 | 1,745,460 |
| sort | 213.4 | 216.8 | +1.6% | 30.1 | 29.7 | 1,659,910 | 1,659,900 |
| sort-nohdr | 38.6 | 38.1 | -1.3% | 14.9 | 14.7 | 727,499 | 727,499 |
| string | 213.9 | 214.2 | +0.1% | 30.5 | 30.0 | 1,920,615 | 1,796,322 |
| string-view-find-if | 644.8 | 621.1 | -3.7% | 33.4 | 33.0 | 16,022,331 | 14,479,696 |
| string-view-pairs | 1680.5 | 557.5 | -66.8% | 31.6 | 31.7 | 31,763,086 | 13,275,413 |
| switch-dense | 260.4 | 254.1 | -2.4% | 14.7 | 14.7 | 34,951,678 | 34,951,678 |
| table | 125.4 | 127.5 | +1.7% | 24.3 | 24.4 | 1,248,889 | 1,248,889 |
| table-nohdr | 63.6 | 64.3 | +1.1% | 15.3 | 15.3 | 854,380 | 854,380 |
| time-trace | 445.4 | 444.8 | -0.1% | 16.0 | 15.9 | 9,000,005 | 9,000,005 |
| trial-rollback | 57.3 | 54.0 | -5.8% | 15.0 | 14.9 | 460,007 | 460,007 |
| vector-alloc | 455.6 | 455.8 | +0.0% | 42.0 | 41.6 | 9,587,322 | 9,587,208 |
| virtual-dispatch | 33.1 | 35.5 | +7.3% | 15.4 | 15.4 | 375,540 | 375,540 |
| wide-struct-read | 320.8 | 317.2 | -1.1% | 30.9 | 30.7 | 4,199,431 | 4,199,431 |
| wide-struct-store | 153.8 | 151.0 | -1.8% | 34.6 | 34.6 | 69,647 | 69,647 |

## The split's own cost

The first design chose an instrumented copy of the VM loop at run time,
once per evaluation, so the unarmed loop had no store and no test. It
still cost 3-4% of the instructions of call-heavy entries unarmed
(callgrind, whole compile: `virtual-dispatch` +4.1%, `trial-rollback`
+3.2%, `churn-tree` +4.0%, `sort-nohdr` +3.0%): with two instantiations
LLVM inlined differently into the loop (`end_life` outlined, `int_bin`
and `store_desig` inlined, more spills in the arithmetic arms). The
stores are now behind the `split` feature, as `op-stats` is, and a stock
build runs the loop main ran. The final build against main, same entries:
`virtual-dispatch` -0.0%, `trial-rollback` +0.1%, `churn-tree` +0.1%,
`sort-nohdr` +0.1%, `wide-struct-store` +0.1% (the remainder is the
compile-time recognition of the fast paths at each call site).

## What is left, and what next

- **The memory model is now 60% of the VM's time** on this workload, and
  most of it is not the comparisons: it is the frames' arguments and
  locals, the by-value `string_view`s copied into each call (`bytes`
  above), and the loads and stores of a machine that keeps every named local
  object in abstract memory. The next step is to keep a local whose address is
  never taken in registers (KBC compile time escape test), which removes
  the frame-memory traffic for most small functions.
- **Tiny calls** (planned in WP-1864, not done): after the fixes,
  frames are 11-16% of the VM's time on both new entries. A light frame
  or KBC-level inlining of one-return functions (`size()`, `data()`,
  `operator[]`, iterator `operator*`/`++`, `lt`/`eq`) would win at most
  that much here, plus the argument copies; `string-view-find-if.cpp`
  is the entry that measures it. Proposed as a follow-up row.
- **Dispatch is 20%.** A closure-compiled tier (each KBC op turned into a
  pre-decoded closure or threaded-code cell) would cut that share, maybe
  by half, so about 10% of the VM's time here. A JIT through the hod
  back end would remove dispatch and keep values in machine registers,
  but each check of the memory model would still be a call into the
  Oven's (safe) memory code, and memory is the larger share. On this
  evidence neither pays off before the register-local and light-frame
  work: they attack the 60% and the 14%, a JIT the 20%.
- **The split itself** is sampled; a hardware-counter profile (perf is not
  available on this host) would make the small shares trustworthy.

## Downstream re-measurement

pending
