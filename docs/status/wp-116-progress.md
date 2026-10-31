# WP-116 progress (canonical expression profiling)

Updated: 2026-10-31

## Baselines (tree cc16ae3, before any change)
- Probe (`kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0`):
  array 100, string_view 164, span 73, utility 34 errors.
  First blocker (array/string_view): `__utility/swap.h:33` alias redefinition.
- callgrind instruction counts (release kiln, `-fsyntax-only`):
  templates_wide 2,189,540,028; templates_deep 679,640,025.

## State
- Survey done; root cause: profiles are `[expr id]` (templates.rs
  `template_arg_expr`, templ/actions.rs `dependent_profile`).

## Next step
- Implement `Ast::profile_expr` (kiln_ast/src/types/profile_expr.rs).
