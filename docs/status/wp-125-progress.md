# WP-125 progress (harness per-test resource limits)

Updated: 2026-10-31

## State: started

| item | state |
|---|---|
| 1 address-space cap on every compiler/oracle/binary invocation, `KILLED` outcome | in progress |
| 2 `tests/stress/` discovery, `mem<=N` directive, dumper cycle regression test | todo |
| 3 `xtask ci` retry once on `error[E0463]` (WP-124) | todo |
| 4 spec + docs + unit tests | todo |

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
