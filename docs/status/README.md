# Status (2026-10-28)

| Milestone | State | Evidence |
|---|---|---|
| M0 bootstrap | done | `cargo xtask ci`, GitHub CI green |
| M1 preprocessor parity | done | [pp-parity.md](pp-parity.md): 291/291 token-wise vs clang 19; [perf.md](perf.md): pp 0.52x–1.42x clang time |
| M2 C-ish subset | done (2026-10-28) | 47 AST golden dirs vs clang, 99 [expr.const] tests, Oven ≈0.02x clang time; IR-diff corpus 92/92 byte-identical to clang -O0 (46 programs × 2 triples); 33 printf `run` tests vs clang |
| M3 classes/templates | planning | first blockers per libc++ header: [libcxx-syntax.md](libcxx-syntax.md) |

libstdc++ consumption track (WP-188): [libstdcxx.md](libstdcxx.md), every public GCC 14 libstdc++ header as a one-line TU (`cargo xtask libstdcxx`): 39/109 ok at c++26, 44/109 at c++20; blocker clusters with corpus repros.

Language features of every standard, C++98 to C++26 (and the M4 checklist): [../STANDARD_SUPPORT.md](../STANDARD_SUPPORT.md), checked by `cargo xtask features`.

Harness through kiln: 565/575 pass, 8 xfail (among them the 5 header-dependent `run` tests: `decltype` in libstdc++'s c++config.h), 2 fail (reflection, contracts).
Workspace: ~1,400 Rust tests across 12 crates, all `#![forbid(unsafe_code)]`.
- `libcxx.md` — libc++ 19 end-to-end status (ROADMAP M3b): sample/full-suite pass rates, failure clusters → WPs.
- `libcxx-m5.md` — libc++ 22 full suite (ROADMAP M5): failure clusters by class (correctness, diagnostic wording, harness, environment), fixes, proposed WPs; the run row is in `libcxx.md`.
- `deviations.md` — deliberate deviations from the oracle where kiln follows the standard over a clang defect, with the spec paragraph and pinning test of each.
- `reflection-spike.md` — WP-267 P2996 design spike behind `-freflection`: what the Oven/SemaBridge boundary absorbed, what had to bend, pre-M6 rows, full-P2996 cost.
- `header-search-syscalls.md` — WP-820 file-system system calls per TU (header search, file loading) against clang 19: fmt, libc++, a 157 `-I` synthetic tree; exact commands to re-run.
