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

## After WP-1907 (light frames, `std::min`/`max`, `char_traits::assign`)

**The premise was partly right.** Frame mechanics are a real share of a
call, but most of a call is not frame-object work. Callgrind on a loop of
200,000 calls of an empty function puts a call at about 249
instructions: about 75 for dispatching `Call`, the callee's `Step` and
`Ret`, 70 in `enter`, 60 in the `Ret` arm, 20 for the outlined
`Memory::release` prologue and 37 for the frame push and pop. A member
call that reads one field (`s.size()`) costs about 456. In a reduced
`string-view-find-if` (40 rounds), `enter`, the return and `release`
are about 8% of the instructions directly and 9-12% with their
dispatch, which matches the 11-17% "frames" share of the split.

Only about a sixth of a call is the frame-object work a function with no
frame object can skip: the memory mark, the frame serial, the release
scan and the `cur_frame` restore. That is what was built (spec/oven.md
"Light frames"). Every frame is still pushed, so steps, the depth limit,
the "in call to" notes, memoization and the profile are unchanged.
`Memory::release` now tests inline whether anything was allocated since
the mark. An empty call goes from 246.8 to 236.8 million instructions
for the 200,000-call loop (-50 per call).

KBC inlining of small callees would remove most of the rest, but it
needs pseudo-frames for the notes and the depth limit, a register remap
at each call site, and an answer for callees compiled after their
callers. It is left for a follow-up row.

The two library rows were the larger win on the WP-1864 entries:

- **`std::min`/`std::max` of two scalars** (WP-1917) run as a builtin
  before the call. Read for its value, as in `substr`'s
  `std::min(__n, this->size() - __pos)`, a register local stays in its
  register and a scalar temporary is never allocated. This was the
  2 x 338 thousand allocations of `string-view-find-if`.
- **`char_traits<C>::assign(c1, c2)`** (WP-1918) is a native
  assignment. It declines through unions, `mutable` members and const
  paths, where libstdc++'s `construct_at` and an assignment could
  differ.

Bench: `bench/oven`, kiln before (46575c3f0) and after on the same host,
7 interleaved runs each (the order flips every run), median times.
`bytes` is the `-fconstexpr-stats` memory counter. Steps change only
where a `std::min`, `std::max` or `assign` call now costs one step.

| entry | before ms | after ms | time | steps | before bytes | after bytes |
|---|---|---|---|---|---|---|
| builtins-memcpy | 24.6 | 21.8 | -11.4% | 10,215 | 65,536 | 65,536 |
| builtins-popcount | 70.6 | 70.8 | +0.3% | 1,600,010 | 0 | 0 |
| builtins-strlen | 21.0 | 20.9 | -0.7% | 14,115 | 4,096 | 4,096 |
| churn-list | 31.8 | 33.0 | +3.6% | 610,005 | 1,600,000 | 1,600,000 |
| churn-mixed | 19.9 | 19.7 | -1.1% | 100,102 | 425,248 | 425,248 |
| churn-string | 449.4 | 446.0 | -0.8% | 6,002,085 | 3,399,552 | 3,399,552 |
| churn-tree | 41.9 | 40.9 | -2.4% | 1,179,557 | 1,572,480 | 1,572,480 |
| churn-vector | 506.1 | 513.7 | +1.5% | 8,398,277 | 33,554,432 | 33,554,432 |
| class-churn | 162.1 | 162.7 | +0.4% | 2,960,095 | 10,240,016 | 10,240,016 |
| fib | 15.6 | 16.1 | +3.6% | 786,024 | 0 | 0 |
| inplace-alloc | 177.1 | 181.9 | +2.7% | 2,500,536 | 1,608,688 | 1,608,688 |
| inplace-union | 182.0 | 181.1 | -0.5% | 2,500,518 | 1,608,672 | 1,608,672 |
| sieve-nohdr | 22.6 | 22.5 | -0.4% | 253,992 | 100,000 | 100,000 |
| sieve | 90.1 | 88.8 | -1.4% | 684,419 | 50,448 | 50,448 |
| soa | 67.2 | 66.0 | -1.7% | 1,745,460 | 73,736 | 73,736 |
| sort-nohdr | 33.8 | 33.2 | -1.6% | 727,499 | 25,600 | 25,600 |
| sort | 201.3 | 204.6 | +1.6% | 1,659,900 | 293,888 | 293,888 |
| string-view-find-if | 605.1 | 557.7 | -7.8% | 14,479,696 to 13,197,908 | 40,767,320 | 31,768,984 |
| string-view-pairs | 539.0 | 536.4 | -0.5% | 13,275,413 to 12,458,145 | 22,804,752 | 22,804,208 |
| string | 204.1 | 205.9 | +0.9% | 1,796,322 to 1,790,230 | 557,008 | 556,312 |
| switch-dense | 240.3 | 246.0 | +2.4% | 34,951,678 | 0 | 0 |
| table-nohdr | 61.8 | 59.4 | -3.9% | 854,380 | 330,240 | 330,240 |
| table | 116.1 | 115.7 | -0.4% | 1,248,889 | 200,560 | 200,560 |
| time-trace | 461.1 | 464.5 | +0.7% | 9,000,005 | 8 | 8 |
| trial-rollback | 70.3 | 76.6 | +8.9% | 460,007 | 1,280,256 | 1,280,256 |
| vector-alloc | 508.1 | 491.9 | -3.2% | 9,587,208 to 9,587,028 | 3,461,664 | 3,460,800 |
| virtual-dispatch | 46.1 | 49.0 | +6.1% | 375,540 | 2,176 | 2,176 |
| wide-struct-read | 332.4 | 336.9 | +1.4% | 4,199,431 | 32,768 | 32,768 |
| wide-struct-store | 151.2 | 155.2 | +2.6% | 69,647 | 16,384 | 16,384 |

`string-view-find-if` runs 7.8% faster, with 22% fewer bytes and 9% fewer
steps. Callgrind on its 41-round reduction counts 2,153,896,951
instructions before and 2,090,017,419 after (-3.0%; the full entry runs
out of address space under valgrind's 3 GB cap).
`string-view-pairs` spends 6% fewer steps and the same time.

The time differences above +2% are noise or layout. Callgrind counts:

- `trial-rollback` (+8.9%): +0.47% instructions. On 15 interleaved runs
  its time is -5.0%.
- `virtual-dispatch` (+6.1%): -3.0% instructions. On 15 runs it is
  +4.6%.
- `fib` (+3.6%): -0.04% instructions. On 15 runs it is -2.4%.
- `churn-list` (+3.6%): +0.17% instructions. On 15 runs it is +2.2%.
- `inplace-alloc` (+2.7%): out of address space under valgrind. On 15
  runs it is -1.6%.
- `switch-dense` (+2.4%), cut to `run(200000)`: +0.64% instructions
  (405,920,838 to 408,520,604). It makes no call, so the extra
  instructions are code generation in `run_loop`, about 0.6 per
  dispatched op. They are not from the inline `release`: a build with
  the old outlined `release` counts the same.

`-fconstexpr-profile-split` (`split` feature builds, 3 interleaved runs
each, medians):

- `string-view-find-if`: VM time 363.6 to 361.6 ms. The memory share
  goes from 56.5% to 55.3%, frames from 17.4% to 17.2%, dispatch from
  21.7% to 21.3%.
- `string-view-pairs`: VM time 357.6 to 357.4 ms. The memory share goes
  from 63.5% to 65.8%, frames from 11.9% to 9.4%, dispatch from 18.5% to
  19.6%.

The split builds show no change in VM time. The stock builds above show
-7.8% for `string-view-find-if`. The split's sampling overhead hides
differences of this size.

What is left, from these counts:

- **KBC inlining of one-return callees**, with pseudo-frames that keep
  the notes, the depth limit and the profile exact. This is most of the
  remaining ~240 instructions of a call.
- **Fewer register moves in KBC**: a compound assignment emits about
  five `Mov`s, and dispatch is the largest single share of a call.
- **Scalar replacement of small class parameters** (WP-1916) can now
  build on light frames.

## After WP-1924 (inlining of small leaf callees)

**What was built.** The VM inlines a direct call of a short leaf function
at its call site (spec/oven.md "Inlining"). At the site's first
execution it appends a copy of the callee's body with the registers
renumbered into the caller's frame, and patches the call to `Inl`. A
fault inside a copy rebuilds the frame the call would have had, so the
notes are unchanged. The depth limit, potential-constant checks and the
per-call hooks fall back to a real call. Memoized callees are not
inlined, so the memo is unchanged. Bodies are capped at 24
instructions: the qualifying library bodies measured in Step 0 are 2 to
16.

**Step 0 (base 8af3bbf6a, temporary counters, not committed).** Dynamic
direct calls whose callee qualifies:

- `string-view-find-if`: 1,791,420 of 4,995,912 calls (36%). Mostly
  `size()` (6 instructions), iterator `operator*` (8) and `operator++` (13).
- `string-view-pairs`: 2,052,860 of 3,097,519 (66%). `size()` and
  `vector::operator[]` (11).
- `sort`: 315,291 of 386,865 (82%). `array::operator[]` (9) and
  `__less` (6).
- `virtual-dispatch`: none. Its 100,000 qualifying calls are virtual.

**What it saves.** Callgrind on a loop of 20,000 calls of a one-field
member function (`s.size()`): an iteration costs 1,258 instructions
before and 1,177 after, so about 80 fewer per call. The loop without the
call costs about 690 per iteration. The call itself (`Call`, `enter`,
the `Ret` arm, the frame push and pop) was about 107 instructions after
light frames. `Inl` (about 18) and `InlRet` (about 8) replace it. The
rest of an inlined call is its body: here a checked read of `this->n`
from abstract memory, which inlining does not remove. The Step 0
estimate (about 10% on the WP-1864 entries) priced a whole light call at
about 30 ns. Only its frame mechanics go, so the gain is smaller.

Bench: `bench/oven`, kiln before (8af3bbf6a) and after on the same host,
7 interleaved runs each (the order flips every run), median process
times. Steps, calls and bytes (`-fconstexpr-stats`) are identical on
every entry.

| entry | before ms | after ms | time | steps | calls | bytes |
|---|---|---|---|---|---|---|
| builtins-memcpy | 22.3 | 22.2 | -0.7% | 10,215 | 2 | 65,536 |
| builtins-popcount | 71.3 | 70.3 | -1.4% | 1,600,010 | 2 | 0 |
| builtins-strlen | 22.8 | 23.1 | +1.4% | 14,115 | 2 | 4,096 |
| churn-list | 31.4 | 31.4 | -0.1% | 610,005 | 1,001 | 1,600,000 |
| churn-mixed | 20.4 | 20.8 | +1.6% | 100,102 | 1 | 425,248 |
| churn-string | 430.5 | 430.3 | -0.1% | 6,002,085 | 65,553 | 3,399,552 |
| churn-tree | 39.4 | 38.7 | -1.6% | 1,179,557 | 393,169 | 1,572,480 |
| churn-vector | 505.8 | 507.1 | +0.3% | 8,398,277 | 65 | 33,554,432 |
| class-churn | 155.8 | 163.7 | +5.1% | 2,960,095 | 800,002 | 10,240,016 |
| fib | 18.1 | 16.4 | -9.0% | 786,024 | 393,012 | 0 |
| inplace-alloc | 166.4 | 163.0 | -2.0% | 2,500,536 | 800,123 | 1,608,688 |
| inplace-union | 174.6 | 176.7 | +1.2% | 2,500,518 | 800,116 | 1,608,672 |
| sieve-nohdr | 22.1 | 23.9 | +8.3% | 253,992 | 2 | 100,000 |
| sieve | 83.5 | 84.2 | +0.8% | 684,419 | 143,485 | 50,448 |
| soa | 64.7 | 67.3 | +3.9% | 1,745,460 | 540,048 | 73,736 |
| sort-nohdr | 35.1 | 33.3 | -5.2% | 727,499 | 93,897 | 25,600 |
| sort | 184.1 | 195.1 | +6.0% | 1,659,900 | 386,719 | 293,888 |
| string-view-find-if | 529.1 | 527.4 | -0.3% | 13,197,908 | 4,995,837 | 31,768,984 |
| string-view-pairs | 499.4 | 506.2 | +1.4% | 12,458,145 | 3,108,522 | 22,804,208 |
| string | 195.9 | 195.1 | -0.4% | 1,790,230 | 720,222 | 556,312 |
| switch-dense | 240.8 | 232.2 | -3.6% | 34,951,678 | 2 | 0 |
| table-nohdr | 56.2 | 54.0 | -3.8% | 854,380 | 582 | 330,240 |
| table | 114.9 | 119.0 | +3.6% | 1,248,889 | 389,727 | 200,560 |
| time-trace | 417.0 | 417.7 | +0.2% | 9,000,005 | 3,000,001 | 8 |
| trial-rollback | 53.5 | 48.6 | -9.1% | 460,007 | 20,001 | 1,280,256 |
| vector-alloc | 421.5 | 399.0 | -5.3% | 9,587,028 | 3,518,820 | 3,460,800 |
| virtual-dispatch | 30.5 | 36.4 | +19.5% | 375,540 | 112,901 | 2,176 |
| wide-struct-read | 305.2 | 305.8 | +0.2% | 4,199,431 | 1 | 32,768 |
| wide-struct-store | 145.1 | 145.4 | +0.2% | 69,647 | 1 | 16,384 |

The flagged differences are noise. On 15 interleaved runs:

- `class-churn` -2.5%, `sieve-nohdr` -2.0%, `soa` -3.0%, `sort` -0.4%,
  `table` -2.1%, `virtual-dispatch` +0.2%.
- `string-view-find-if` -0.7%, `string-view-pairs` -0.1%.

Callgrind counts fewer instructions on each of them:

- `virtual-dispatch` -0.72%.
- `sieve-nohdr` -0.61%.
- `class-churn`, cut to `churn(8, 2000)`: -0.20%.
- `sort` -1.24%.
- `table` -3.07%.
- `soa`, cut to `N = 1024`: -2.83%.

Evaluation time alone (`-fconstexpr-stats` `wall-us`, 9 interleaved
runs, medians):

| entry | before ms | after ms | change |
|---|---|---|---|
| string-view-find-if | 304.9 | 300.0 | -1.6% |
| string-view-pairs | 310.3 | 312.2 | +0.6% |
| sort | 29.2 | 28.2 | -3.4% |
| table | 24.8 | 22.9 | -7.5% |
| soa | 46.4 | 44.7 | -3.7% |

Per round (callgrind, 40 rounds against 1 for `string-view-find-if`, 50
against 1 for `string-view-pairs`), the evaluation runs about 1.8% and
1.6% fewer instructions.

`-fconstexpr-profile-split` (`split` feature builds, 5 interleaved runs
each, medians):

- `string-view-find-if`: VM time 349.4 to 330.6 ms (-5.4%). Frames go
  from 17.3% to 12.5%, dispatch from 20.3% to 23.5%, memory from 56.1%
  to 59.1%.
- `string-view-pairs`: VM time 341.5 to 330.4 ms (-3.3%). Frames go from
  10.3% to 7.9%; dispatch stays at 18.6%.

The split's per-phase stores make each call more expensive, so these
builds show more of the gain than the stock builds do.

What is left, from these counts:

- **Memory reads of small class objects.** In an inlined `size()` most
  of the cost is the checked load through `this`. Scalar replacement
  (WP-1916) would remove it, and inlining now puts the callee's loads
  in the caller's code.
- **Non-leaf callees.** Bodies that call (`substr`, `__sv_check`), name a
  global or end a lifetime (`operator==`) are the largest refusals in
  Step 0. Inlining them needs nested pseudo-frames.
- **The memo on trivial functions.** A memoized one-return `id(i)` call
  costs about 100 ns, against about 35 ns for a light call: for a
  function this small the memo lookup costs more than the evaluation
  it saves.
