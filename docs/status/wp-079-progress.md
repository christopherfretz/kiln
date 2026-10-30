# WP-079 progress (codegen for classes)

Updated: 2026-10-30 19:00 EDT

## State
- Builds; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean.
- IR-diff corpus exact vs clang on aarch64 + x86_64 (7 new cases):
  class_structors, class_temporaries, class_copy, class_members,
  class_aggregate_bases, class_static_members, class_member_pointers.
- Member pointers implemented in `crates/kiln_codegen/src/memptr.rs`.
- WP-079 xfails removed (6 files); all pass at runtime with clang-identical IR.
- spec/codegen.md §7 and spec/abi.md §8.6 "Landed (WP-079)" written.
- Full `cargo xtask ci` running (log in session scratch); result pending.

## Next step
Check `cargo xtask ci` result (fmt, clippy, test, corpus both modes,
conformance >= 1121/1147 clang, >= 1129/1147 kiln). Fix any failure, commit,
hand back. Do not push or merge.
