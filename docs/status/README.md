# Status (2026-10-28)

| Milestone | State | Evidence |
|---|---|---|
| M0 bootstrap | done | `cargo xtask ci`, GitHub CI green |
| M1 preprocessor parity | done | [pp-parity.md](pp-parity.md): 291/291 token-wise vs clang 19; [perf.md](perf.md): pp 0.52x–1.42x clang time |
| M2 C-ish subset | done (2026-10-28) | 47 AST golden dirs vs clang, 99 [expr.const] tests, Oven ≈0.02x clang time; IR-diff corpus 92/92 byte-identical to clang -O0 (46 programs × 2 triples); 33 printf `run` tests vs clang |
| M3 classes/templates | planning | first blockers per libc++ header: [libcxx-syntax.md](libcxx-syntax.md) |

Harness through kiln: 565/575 pass, 8 xfail (among them the 5 header-dependent `run` tests: `decltype` in libstdc++'s c++config.h), 2 fail (reflection, contracts).
Workspace: ~1,400 Rust tests across 12 crates, all `#![forbid(unsafe_code)]`.
