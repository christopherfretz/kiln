# WP-082 progress — defaulted comparison operators

Branch `wp/082-defaulted-comparisons`, worktree `.worktrees/wp-082`.

## Baseline (main @ 45fbbab)
- `<compare>` 50 errors, `<utility>` 83 errors.
- `<compare>` WP-082-owned: 6 `operator '<=>'` not supported, 3 "defaulted
  comparison operators", part of 12 "selecting the constructor" (ordering.h
  static member definitions / `_CmpUnspecifiedParam`).

## Plan
1. `class/compare.rs`: comparison-category lookup/validation (`std::*_ordering`).
2. Builtin `<=>` on scalars (sema, Oven, codegen).
3. Declaration checks + deferral of defaulted comparisons; implicit `==` from `<=>`.
4. Deletedness / category deduction / constexpr.
5. Implicit definitions (clang-shaped bodies) on odr-use and `ensure_body`.
6. Tests + spec + probe.

## Status
- Exploration done; implementation starting.

## Next step
- Step 1.
