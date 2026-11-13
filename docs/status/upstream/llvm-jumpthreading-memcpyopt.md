# Upstream report: JumpThreading is super-linear on one long chain of invokes

Status: draft, **not filed**. It is the issue text for the LLVM issue
tracker (F22 in docs/status/perf-reflection.md). It is
clean-room: the reproducer is public and written for this report, and
every number below comes from clang's own IR, so the report does not
depend on kiln. Kiln works around the shape by outlining (WP-641,
spec/landed/codegen/WP-641.md); the LLVM cost is still there for clang
and for any other frontend.

---

**Title:** [JumpThreading] Quadratic-or-worse time on a function with
thousands of sequential invokes after inlining (`-O2`)

**Component:** `llvm:optimizations` (`llvm/lib/Transforms/Scalar/JumpThreading.cpp`;
also MemCpyOpt, see "Second term")

**Versions:** LLVM 19.1.7 (Debian `opt-19`, `clang++-19`) and LLVM 22.1.8
(Debian `opt-22`), aarch64-unknown-linux-gnu. Kiln's driver pins LLVM 19
(`crates/kiln_driver/src/toolchain.rs`, `opt-19`/`llc-19`).

**Reproducer:** `bench/frontend/registration-wide-obj.cpp` in this
repository (58 lines, no headers). One function, `register_all`, makes
N calls `m.def("fK", lambda, K % 4, K % 3 != 0, r::arg("x"))`. Each call
inlines a member template that fills a 40-byte descriptor through a few
folded branches, takes the lambda's function pointer, and calls an
external `init`. The by-value `r::arg` temporary has a non-trivial
destructor, so every call is an `invoke` with its own landing pad. The
file has N = 4,000; N = 1,000 and 2,000 keep the first one or two
`B1000(...)` groups in `register_all`.

```sh
clang++ -std=c++20 -O2 -S -emit-llvm -Xclang -disable-llvm-passes \
  registration-wide-obj.cpp -o reg.ll
opt -passes='default<O2>' -time-passes reg.ll -o /dev/null
```

**IR shape** (`register_all` before `opt`, clang 19 IR; kiln's IR has the
same counts):

| N | Instructions | Blocks | Landing pads | Invokes | Allocas |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 18,010 | 2,002 | 1,000 | 1,000 | 2,003 |
| 2,000 | 36,010 | 4,002 | 2,000 | 2,000 | 4,003 |
| 4,000 | 72,010 | 8,002 | 4,000 | 4,000 | 8,003 |

The module has 16,006 functions at N = 4,000; every other function is
small. After the inliner, `register_all` is one long chain: each
binding's inlined body is a few blocks of folded conditionals ending in
an `invoke` whose normal edge continues to the next binding, and the
entry block holds every descriptor's `alloca`.

**Measurements** (`opt -passes='default<O2>' -time-passes` on clang 19's
IR; wall time from `/usr/bin/time`, single runs, sequential, on a shared
8-core host):

| N | opt-19 wall | opt-19 RSS | JumpThreading | share | MemCpyOpt | opt-22 wall | opt-22 JumpThreading |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1,000 | 5.36 s | 116,788 KB | 3.81 s | 77.2% | 0.01 s | 2.90 s | 1.56 s (61.5%) |
| 2,000 | 21.78 s | 168,972 KB | 18.75 s | 89.3% | 0.02 s | 12.99 s | 10.16 s (82.8%) |
| 4,000 | 124.82 s | 273,920 KB | 117.9 s | 95.9% | 0.12 s | not run | not run |

Growth of JumpThreading time per doubling of N: 2.30 and 2.65 as an
exponent in LLVM 19, and 2.70 in LLVM 22. Every other pass stays linear:
the rest of the pipeline is 1.1 s, 2.2 s and 5.0 s. LLVM 22 is faster at
a fixed size but grows faster. Kiln's own IR for the same file gives
JumpThreading 110 s and 85 s at N = 4,000 (two runs) under `opt-19`, the
same order as clang's.

**Which part of the pass.** Only the JumpThreading term reproduces here.
With `-fno-exceptions` the function is a single block before inlining,
and JumpThreading still takes 104 s of `opt`'s 110 s at N = 4,000
(kiln's IR). It also survives turning every invoke into a call, removing
the lifetime markers and sharing landing pads; each of these variants
stays within 25% of the baseline. So the cost is in the long
straight-line chain left after inlining, not in the EH edges. Our
reading, from the IR variants and not from a profile of `opt`, is that
the cost is in JumpThreading's block merging along the inlined chain:
each merge moves an ever-growing block that begins with the entry
block's allocas. Lazy value info and dominator-tree updates repeated per
merge are the likely source, but that has not been isolated. On the
production unit described below, the limits
`-jump-threading-threshold=0`,
`-jump-threading-implication-search-threshold=0` and
`-memssa-check-limit=10` left JumpThreading at 60.8% and MemCpyOpt at
32.3% of `opt`. The cost is therefore not in the duplication or
clobber-walk budgets that these flags cap.

**Second term (MemCpyOpt), not reproduced by this file.** A larger
production translation unit of the same shape (one function, 6,455
landing pads, 10,762 allocas, 17,221 blocks) spends 38.7% of its `opt`
time in one MemCpyOpt run and 52.9% in one JumpThreading run. On that
IR, MemCpyOpt does not depend on the exception edges: 86 s with every
invoke a call. The likely cause is `tryMergingIntoMemset` updating a
MemorySSA that spans the whole function, once per run of field stores.
This reproducer's descriptors are too short to trigger it (MemCpyOpt is
under 0.2 s above). In LLVM 22 the same production IR moves time from
JumpThreading to MemCpyOpt (43.2%), LICM (39.9%) and LoopRotate (11.6%).

**Comparison:** GCC 16.1 `-O2` compiles the production unit in 75.5 s,
against 432.5 s for clang 19.

**Expected:** time roughly linear in the function's size, or a size
bound on the block-merging work that keeps a function of 8,000 blocks
in seconds.

**Workaround in a frontend:** split very long statement lists into
`noinline` helpers. Kiln does this at 256 call-containing statements:
`opt` drops from 116 s to 6.2 s on this reproducer.
