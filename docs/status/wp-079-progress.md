# WP-079 progress (codegen for classes)

Updated: 2026-10-30 19:10 EDT

## State
- Builds; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean.
- IR-diff corpus exact vs clang on aarch64 + x86_64 (7 new cases):
  class_structors, class_temporaries, class_copy, class_members,
  class_aggregate_bases, class_static_members, class_member_pointers.
- Member pointers implemented in `crates/kiln_codegen/src/memptr.rs`.
- WP-079 xfails removed (6 files); all pass at runtime with clang-identical IR.
- spec/codegen.md §7 and spec/abi.md §8.6 "Landed (WP-079)" written.
- `cargo xtask ci` green (exit 0): fmt, clippy, tests (ir_vs_clang and
  mangle_vs_clang ok), conformance clang self-check 1121/1147, kiln 1135/1147
  (was 1129), 0 fail / 0 xpass.

## Next step
Done. Awaiting supervisor review/merge. Do not push or merge from the worktree.
