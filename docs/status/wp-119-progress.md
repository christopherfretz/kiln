# WP-119 progress (`decltype(expr)::` nested-name-specifier)

Updated: 2026-10-31

## State
- Sema/AST side builds: `AstBuilder::unlinked_alias`,
  `Sema::act_on_nested_name_decltype` (non-dependent -> class/enum scope,
  dependent -> a shared dependent-marker alias with `Nns::Type{decltype}`),
  `TemplState::decltype_scope`.
- Parser side in progress: a one-slot "decltype scope" annotation (the
  parsed `decltype(...)` qualifier pending at its `::`), consumed by
  `scan_name_qual_at` / `parse_nested_name_nns`.

## Next step
Wire the annotation into decl-specifiers, primary expressions, tentative
statement/condition checks, template arguments, member pointers; tests.
