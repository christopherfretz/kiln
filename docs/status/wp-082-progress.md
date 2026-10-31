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
- Steps 1-2 done: `std::*_ordering` recognition/validation, builtin `<=>`
  in sema, the Oven and codegen.
- Steps 3-4 done (`class/compare/{default,analyze}.rs`): clang's
  `CheckExplicitlyDefaultedComparison` + `DefaultedComparisonAnalyzer`
  (deletedness, category deduction, constexpr, access via
  `cmp_member_access`), implicit `operator==` from a defaulted `<=>`,
  checks at class completion (primaries, then secondaries). Scratch
  diffs e1/e2/e5/e7 match clang; e3 off by RetLoc column (spec gap).
- Pre-existing gaps seen (not WP-082): Oven "non-constexpr function" note
  points at the operand start instead of the operator; explicit object
  parameters unsupported in the parser.
- Step 5 done (`class/compare/synth.rs`): clang-shaped bodies on odr-use
  (`mark_function_referenced`, before the exception spec) and from
  `ensure_body`; implicit exception spec from the body (built unevaluated
  when not yet defined); deleted-function notes via the analyzer.
  Scratch d1 (base + int + float[2], `<=>`, `<`, implicit `==`) matches
  clang except `nrvo_candidate` (clang's JSON omits it: <=> goldens need
  `skip(clang)` checked against the text dump).
- Side fixes needed by the goldens: comparison category types print
  `'std::strong_ordering'` (Elaborated with the namespace qualifier,
  `scoped_class_type`); CXXOperatorCallExpr `adl` when the operator was
  found only by ADL; a defaulted comparison's exception spec is resolved
  before its callee DeclRefExpr is built.

## Next step
- Rebuild, re-diff d1; `git merge main`; tests in the 5 conformance dirs;
  class-template instantiation of defaulted comparisons; probe; spec.
