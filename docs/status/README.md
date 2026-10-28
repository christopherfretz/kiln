# Status (2026-10-28)

| Milestone | State | Evidence |
|---|---|---|
| M0 bootstrap | done | `cargo xtask ci`, GitHub CI green |
| M1 preprocessor parity | done | [pp-parity.md](pp-parity.md): 291/291 token-wise vs clang 19; [perf.md](perf.md): pp 0.52x–1.42x clang time |
| M2 C-ish subset | codegen pending | 47 AST golden dirs vs clang, 99 [expr.const] tests, Oven ≈0.02x clang time; `run` tests after WP-052 |
| M3 classes/templates | planning | first blockers per libc++ header: [libcxx-syntax.md](libcxx-syntax.md) |

Harness through kiln: 533/541 conformance tests (remaining: `run` tests, reflection, contracts).
Workspace: ~1,400 Rust tests across 12 crates, all `#![forbid(unsafe_code)]`.
