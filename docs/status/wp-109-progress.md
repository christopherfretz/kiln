# WP-109 progress (concepts + alias follow-ups)

Branch `wp/109-concepts-alias-followups`. Living file; deleted at merge.

## Item 1 — dependent member templates as template arguments: DONE
- Parser: `X<T>::template M` (and `X<T>::M` naming a member template of a
  dependent specialization) as a template template argument is a
  `DependentIdentifier` template name, re-looked-up at substitution;
  inside `X` itself (current instantiation) it resolves now to a
  `Qualified` name (`kiln_parse/src/{parser,templ}.rs`).
- Substitution: a template template parameter replaced by a `Qualified`
  name yields its underlying declaration (clang prints `apply<int>`).
- `unqualified()` peels sugar when qualifiers are buried in it
  (`__remove_cvref(T)` with `T = const int` kept `const`).
- Alias template redeclaration with the same type is accepted
  (`sema_redefinition_different_typedef` gained a third form).
- Tests: `templates/alias/temp.alias-dependent-member-template-arg.cpp`,
  `temp.alias-dependent-template-template-arg-ast.cpp` (+ golden),
  `temp.alias-redeclaration.cpp`. xtask clang-ast accepts template-name
  arguments of a specialization.
- Probe (errors before -> after): type_traits 2 -> 0; array 180 -> 178;
  span 116 -> 114; string_view 332 -> 330.
  New first blockers: array/string_view `__utility/swap.h:33` alias
  template `__swap_result_t` redeclared with `__enable_if_t<expr>`: equal
  dependent expressions are not the same type because expression
  profiles are ExprId stubs (spec/types.md §7.2 `profile_expr` not
  implemented); span `__compare/ordering.h:62` defaulted comparisons
  (WP-082).

## Item 2 — tighten the 073 fallback: IN PROGRESS
- `sat_atom`: an unimplemented substitution is now a hard error (the
  captured diagnostic is emitted); a value-dependent result is satisfied
  only when the levels are dependent (retained level / dependent arg),
  else a hard "not yet implemented" error. `req_expr` likewise.
- Corpus fallout: `templates/concepts/temp.constr.concept-std-like.cpp`
  hits `__is_convertible` (unimplemented trait) — needs the trait.
- Fold-expanded constraints `(C<Ts> && ...)` evaluate correctly; notes
  differ from clang 19 (clang expands the fold per element:
  "because 'long' does not satisfy 'Int'"; kiln "'... && ...' evaluated
  to false"). TODO: `sat_fold` in concepts/sat.rs + tests.

## Item 4 — note locations: mostly DONE
- type-requirement note at the type (`Requirement::Type(QualType, Loc)`),
  noexcept note at the keyword (`Compound { noexcept: Option<Loc> }`),
  member access after substitution at the `.`/`->`
  (`templates.member_op_loc` side table). Verified vs clang on scratch.
- `static_assert(requires{...})` prints the expression like clang
  (`AstNames::expr` RequiresExpr arm; test
  `templates/concepts/expr.prim.req-static-assert-print.cpp`).
- TODO: non-bool atom at concept definition; ambiguous-atomic note.

## Items 3, 5, 6, 7: pending
