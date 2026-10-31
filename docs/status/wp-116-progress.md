# WP-116 progress (canonical expression profiling)

Updated: 2026-10-31

## Baselines (tree cc16ae3, before any change)
- Probe (`kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0`):
  array 100, string_view 164, span 73, utility 34 errors.
  First blocker (array/string_view): `__utility/swap.h:33` alias redefinition.
- callgrind instruction counts (release kiln, `-fsyntax-only`):
  templates_wide 2,189,540,028; templates_deep 679,640,025.

## State
- Done: `Ast::profile_expr` + unit tests; canonical dependent types and
  template args use it; `templ/equiv.rs` (`exprs_equivalent`,
  `equivalent_signatures`, `check_template_heads_match`); typedef
  redefinition per clang; 27 `templates/equiv` tests (26 + 1 xfail);
  spec landed blocks (types.md §7.2, sema-templates.md §11.2).
- Probe after (debug kiln at d8c63f2): array 96, string_view 160,
  span 69, utility 32. swap.h:33 gone. New first blockers:
  `__iterator/iterator_traits.h:123:95` (`decltype(...)::value` as NNS,
  parser) for array/string_view; defaulted comparisons (WP-082) for
  span/utility.

- Bench after (release, bc15296): templates_wide 2,171,789,905
  (-0.8%), templates_deep 678,675,597 (-0.1%). Before the meter fix
  the numbers were +9.7% and +1.5%: the per-frame `node_live_bytes`
  allocated a `String` per arena vector once LLVM stopped eliding the
  allocations. Fixed with `VecDims` (spec/ast.md §2.3).
- Quick CI (d8c63f2): 1411 passed, 15 xfail.

## Next step
- Done; handed back. Full CI (bc15296): clang 1398 + 3 xfail + 25 skipped; kiln 1411 + 15 xfail.
