# WP-125 progress (harness per-test resource limits)

Updated: 2026-10-31

## State: done (ready for review)

| item | state |
|---|---|
| 1 address-space cap on every compiler/oracle/binary invocation (test + bench), `KILLED` outcome | done |
| 2 `tests/stress/` discovery, `mem<=N` directive, dumper cycle regression test | done (the test is `xfail(kiln)` for a dump-fidelity gap; KILLED is never masked) |
| 3 `xtask ci` retry once on `error[E0463]` (WP-124) | done |
| 4 spec + docs + unit tests | done |
| exit gates: fmt, clippy, `cargo test -p xtask` (123 passed), kiln corpus (1589 passed, 15 xfail, 0 killed), main merged | done; full `cargo xtask ci` green (clang self-check 1574 passed, 4 xfail, 26 skipped, 0 killed; kiln 1589 passed, 15 xfail, 0 killed) |

## Mechanism (decided)
- `std::process::Command` has no rlimit API and `#![forbid(unsafe_code)]`
  rules out `pre_exec`/FFI, so each capped child is spawned as
  `sh -c 'ulimit -v "$1" || exit 125; shift; exec "$@"' kiln-mem-limit <KiB> <program> <args...>`
  (POSIX sh, dash on Debian). `exec` keeps the pid, so timeouts and RSS
  measurement are unchanged.
- kiln reserves its compile thread's stack (1 GiB default,
  `-fkiln-stack-size=`) as address space; the harness adds that reservation
  to the cap for kiln invocations, so a budget measures what the compile
  allocates.
- Kill detection: `util::process::out_of_memory` (allocation-failure
  signatures in stderr; abort markers only with a signal / exit >= 128).

## Findings for the supervisor
- kiln exits 70 with no diagnostic when it cannot spawn its compile thread
  (cap below the stack reservation): `crates/kiln_driver/src/main.rs`.
- `tests/stress/dump-friend-template-cycle.cpp`: kiln's dump of a
  befriended class template inside an instantiation shows the pattern
  (U at depth 1, no specialization list); clang shows it instantiated.
  No cycle and no blow-up (0.02 s); recorded as `xfail(kiln)`.
