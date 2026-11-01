# WP-148 — xtask as a jobserver client

Branch `wp/148-xtask-jobserver`. xtask now honours a GNU make fifo jobserver
inherited through `MAKEFLAGS` (`--jobserver-auth=fifo:<path>`, exported by
`scripts/env.sh` when the host pool is live). Contract: `spec/harness.md`
"Jobserver".

## What changed

- `xtask/src/jobs.rs`: the client. `global()` parses `MAKEFLAGS` once;
  `Client::acquire()` returns a `Token` released by `Drop`. The implicit
  token is handed out first; further tokens are pool bytes read by one
  lazily started helper thread (workers wait on a condvar, so a freed
  implicit token or a token released by a sibling wakes them; a byte nobody
  needs any more goes straight back). No jobserver, the descriptor forms, a
  non-fifo or an unopenable path: `acquire()` is free and `-j` alone bounds
  parallelism, as before. No `unsafe`, no new dependency (the `jobserver`
  crate's `Client::from_env` is an `unsafe fn`, and xtask is
  `#![forbid(unsafe_code)]` through the workspace lints).
- `xtask/src/util/process.rs`: `run_with_input` (so `run`) and
  `run_measured` hold one token for the child's lifetime, taken before the
  timeout / wall clock starts. Every compiler, oracle and test-binary
  process of `xtask test`, `xtask libcxx` and `xtask bench` goes through
  these two functions.
- Run headers name the pool (`8 jobs (jobserver fifo:…)`); `-j` help text
  says it is an upper bound under a jobserver.

## Verification

Concurrency, private 2-token pool (`.jobs-test/pool.fifo` in the worktree,
held open and primed with `++` like `scripts/jobserver.sh`), two concurrent
`cargo xtask test -j 8` (full corpus with the clang oracle, 1776 tests
each), sampling every 50 ms the direct children of the two xtask processes
(every token holder is one: the `ulimit` wrapper `exec`s, so the pid is the
compiler's / binary's):

| samples | max children | max kiln | mean children | bound (pool + xtasks) |
|---|---|---|---|---|
| 1504 | **4** | 4 | 3.58 | 2 + 2 = 4 |

Distribution of in-flight children: 0: 11, 1: 44, 2: 37, 3: 386, 4: 1026
samples. Both runs passed (1766 pass, 10 xfail); after both exited the pool
held its 2 idle tokens again. (Without the client the same two runs would
have had up to 16 children.)

Throughput with no `MAKEFLAGS`, `-j 2`, full corpus, the pre-WP xtask
(`0b5156c`) against this branch, interleaved on a loaded shared host (load
average ~11 on 24 CPUs):

| run | base | new |
|---|---|---|
| 1 | 104.0 s | 83.0 s |
| 2 | 88.5 s | 109.9 s |
| mean | 96.3 s | 96.5 s |

The means agree within 0.2 %; single runs vary ±13 % with host load. With no
jobserver `acquire()` returns at its first `let … else` without locking.

Unit tests (`jobs::tests`): MAKEFLAGS parsing, the free client, non-fifo
rejection, a peak of at most pool + 1 with every byte returned, a freed
implicit token waking a waiter, and a panicking holder returning its token.

## Not done / follow-ups

- A hard kill of xtask (SIGTERM from `timeout`, SIGINT from an interrupt,
  SIGKILL) loses the pool tokens it held at that moment: observed while
  killing a measurement run (the private pool dropped from 2 to 0). cargo
  and rustc have the same property, so the host pool already leaks under
  interrupts. Options: a signal handler in xtask (needs a crate such as
  `signal-hook`/`ctrlc`, unsafe inside the dependency), and/or a
  `scripts/jobserver.sh refill` that tops the pool back up when no client
  is running.
- `--jobserver-auth=R,W` / `--jobserver-fds=R,W` are ignored (they need
  inherited descriptors; reopening `/proc/self/fd/R` could pick up an
  unrelated fd if the parent did not pass them).
