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

## After WP-1908 (register locals)

**The premise was half wrong.** The first bullet of "What is left" says
the machine keeps every named local in abstract memory. It does not: since
M2 the KBC pre-scan has kept every initialized scalar local and every scalar
parameter whose address it never sees in a VM register (spec/oven.md
"Register locals"). A temporary count of `Op::Alloc` per origin (not
committed) showed where the memory share of the two WP-1864 entries
comes from:

- `string-view-pairs`: by-value `basic_string_view` parameters (`__x` and
  `__y` of `operator==`, `__str` of `compare`, 280-370 thousand each).
  These are class objects. The only scalar locals in memory are bound to a
  reference parameter (`char_traits::assign`'s `const&`,
  `__iterator_category`'s `const&`, `_M_create`'s `size_type&`), 1-3
  thousand each.
- `string-view-find-if`: `substr`'s `__n` and a `size_t` temporary, about
  340 thousand each, both bound to the `const&` parameters of
  `std::min<size_t>`. The rest are class objects: `string_view` and
  `__normal_iterator` parameters, closures, and `equal_to<>` temporaries.

None of these are in scope, so both entries are unchanged: steps, bytes
and calls are identical. Two things were in scope and are fixed:

- **`std::move` and its kind no longer count as an escape.** This covers
  `std::forward`, `forward_like`, `move_if_noexcept` and `as_const`. The
  Oven already ran these without a call, but the scan treated their
  argument as bound to a reference, so the local went to memory. This
  happened to `std::swap`'s `__t` (`b = std::move(__t)`) in libc++ `sort`
  and in `vector-alloc`.
- **A scalar declared without an initializer is a register when it is
  assigned before any other use.** This is libc++ `string`'s `__p`, `__sz`
  and `__cap`, assigned on both branches of an `if`. The analysis is
  deliberately narrow: it walks only straight-line code and `if`
  branches, it gives up at any label in scope, and the assignment must
  not read the variable. The uninitialized-read diagnostics are
  unchanged, because a variable that might be read before it is
  assigned stays in memory.

Bench: `bench/oven`, kiln before (599269140) and after on the same
host, 7 interleaved runs each (before and after alternate; the order
flips every run), median times. The steps are identical in every run
of every entry. `bytes` is the `-fconstexpr-stats` memory counter.

| entry | before ms | after ms | time | before MB | after MB | steps | before bytes | after bytes |
|---|---|---|---|---|---|---|---|---|
| builtins-memcpy | 22.4 | 23.0 | +2.3% | 14.9 | 15.0 | 10,215 | 65,536 | 65,536 |
| builtins-popcount | 68.4 | 69.5 | +1.7% | 14.8 | 14.9 | 1,600,010 | 0 | 0 |
| builtins-strlen | 22.6 | 21.8 | -3.5% | 15.1 | 15.0 | 14,115 | 4,096 | 4,096 |
| churn-list | 33.0 | 34.4 | +4.1% | 15.1 | 14.9 | 610,005 | 1,600,000 | 1,600,000 |
| churn-mixed | 22.0 | 22.3 | +1.3% | 15.2 | 14.8 | 100,102 | 425,248 | 425,248 |
| churn-string | 429.1 | 436.4 | +1.7% | 15.7 | 15.4 | 6,002,085 | 3,399,552 | 3,399,552 |
| churn-tree | 37.3 | 37.6 | +0.9% | 15.6 | 15.4 | 1,179,557 | 1,572,480 | 1,572,480 |
| churn-vector | 499.4 | 502.5 | +0.6% | 15.7 | 15.5 | 8,398,277 | 33,554,432 | 33,554,432 |
| class-churn | 170.6 | 171.1 | +0.3% | 17.6 | 17.3 | 2,960,095 | 10,240,016 | 10,240,016 |
| fib | 18.7 | 19.2 | +2.6% | 14.8 | 14.7 | 786,024 | 0 | 0 |
| inplace-alloc | 180.9 | 181.2 | +0.1% | 26.4 | 26.2 | 2,500,536 | 1,608,688 | 1,608,688 |
| inplace-union | 189.0 | 188.3 | -0.4% | 26.4 | 26.2 | 2,500,518 | 1,608,672 | 1,608,672 |
| sieve | 92.4 | 94.9 | +2.7% | 22.4 | 22.2 | 684,419 | 50,448 | 50,448 |
| sieve-nohdr | 24.4 | 23.9 | -1.8% | 15.0 | 14.8 | 253,992 | 100,000 | 100,000 |
| soa | 72.6 | 71.8 | -1.2% | 15.5 | 15.4 | 1,745,460 | 73,736 | 73,736 |
| sort | 213.7 | 208.5 | -2.4% | 30.1 | 30.0 | 1,659,900 | 648,080 | 293,888 |
| sort-nohdr | 36.2 | 35.7 | -1.4% | 15.0 | 14.9 | 727,499 | 25,600 | 25,600 |
| string | 222.9 | 221.1 | -0.8% | 30.1 | 30.3 | 1,796,322 | 626,512 | 557,008 |
| string-view-find-if | 637.7 | 630.4 | -1.1% | 32.8 | 32.7 | 14,479,696 | 40,767,320 | 40,767,320 |
| string-view-pairs | 567.3 | 570.3 | +0.5% | 31.0 | 31.1 | 13,275,413 | 22,804,752 | 22,804,752 |
| switch-dense | 235.8 | 243.8 | +3.4% | 14.9 | 14.7 | 34,951,678 | 0 | 0 |
| table | 140.5 | 147.1 | +4.6% | 24.4 | 24.0 | 1,248,889 | 200,560 | 200,560 |
| table-nohdr | 55.2 | 55.5 | +0.7% | 15.4 | 15.1 | 854,380 | 330,240 | 330,240 |
| time-trace | 424.7 | 436.7 | +2.8% | 16.2 | 15.6 | 9,000,005 | 8 | 8 |
| trial-rollback | 47.3 | 47.8 | +1.3% | 15.2 | 15.0 | 460,007 | 1,280,256 | 1,280,256 |
| vector-alloc | 412.2 | 421.9 | +2.3% | 41.6 | 41.5 | 9,587,208 | 3,462,096 | 3,461,664 |
| virtual-dispatch | 30.2 | 30.7 | +1.7% | 15.5 | 15.5 | 375,540 | 2,176 | 2,176 |
| wide-struct-read | 307.7 | 298.1 | -3.1% | 31.1 | 31.0 | 4,199,431 | 32,768 | 32,768 |
| wide-struct-store | 142.0 | 143.2 | +0.9% | 34.9 | 34.8 | 69,647 | 16,384 | 16,384 |

`sort` allocates 55% fewer bytes, `string` 11% fewer. Callgrind counts
the instructions of a whole compile; nothing above +0.1% is real:

- `table`: +0.02% (1,179,767,834 to 1,180,037,925). On a second set of
  15 interleaved runs its time is +2.1%.
- `switch-dense`, cut to `run(200000)` because the full entry runs out
  of address space under valgrind's 3 GB cap: +0.0001% (405,908,685 to
  405,909,099). On 15 runs its time is +3.6%. Its bytes are 0 in both
  builds, so the change does not reach this entry; the time difference
  is code layout.
- `sort`: -2.0% (1,860,751,610 to 1,823,082,991).

`-fconstexpr-profile-split` (`split` feature builds, 3 runs each, medians):

- `string-view-pairs`: VM time 381.4 to 365.6 ms. The memory share goes
  from 64.8% to 62.2%, frames from 11.5% to 12.3%, dispatch from 18.0%
  to 19.3%.
- `string-view-find-if`: VM time 425.3 to 410.3 ms. The memory share
  goes from 57.1% to 53.5%, frames from 16.9% to 16.8%, dispatch from
  18.5% to 18.6%.

Neither entry allocates differently, so these shifts are sampling and
layout noise in the `split` builds. The stock builds above put these two
entries at +0.5% and -1.1%.

The budget test `tests/conformance/expr.const/budget/register-locals.cpp`
is a selection sort whose swap uses `std::move`, with locals assigned
before use. It spends the same 3145 steps before and after, and 128
bytes after instead of 4864.

What is left, from these counts:

- **Scalar replacement of small trivially copyable class parameters**,
  such as `string_view` by value. This is the bulk of the memory share
  of both WP-1864 entries. Member calls take `this`, so it needs the
  light frames and inlining of WP-1907 first.
- **`std::min` and `std::max` of two scalars** as a library fast path,
  or a callee analysis of `const T&` parameters. In
  `string-view-find-if`, `substr` allocates `__n` and a temporary on
  every call, 2 x 338 thousand allocations.
- **`char_traits<C>::assign(c1, c2)`** as a fast path, for libstdc++
  `push_back`'s `__c`.
