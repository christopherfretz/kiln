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

## Item 2 — tighten the 073 fallback: DONE (sat_fold deferred)
- `sat_atom`: an unimplemented substitution is now a hard error (the
  captured diagnostic is emitted); a value-dependent result is satisfied
  only when the levels are dependent (retained level / dependent arg),
  else a hard "not yet implemented" error. `req_expr` likewise.
- Corpus fallout: `templates/concepts/temp.constr.concept-std-like.cpp`
  hits `__is_convertible` (unimplemented trait) — needs the trait.
- Fold-expanded constraints `(C<Ts> && ...)` evaluate correctly; notes
  differ from clang 19 (clang expands the fold per element:
  "because 'long' does not satisfy 'Int'"; kiln "'... && ...' evaluated
  to false"). `sat_fold` (per-element notes) deferred; recorded as a
  known divergence in spec/sema-templates.md §11.2 (Landed WP-109).

## Item 4 — note locations: mostly DONE
- type-requirement note at the type (`Requirement::Type(QualType, Loc)`),
  noexcept note at the keyword (`Compound { noexcept: Option<Loc> }`),
  member access after substitution at the `.`/`->`
  (`templates.member_op_loc` side table). Verified vs clang on scratch.
- `static_assert(requires{...})` prints the expression like clang
  (`AstNames::expr` RequiresExpr arm; test
  `templates/concepts/expr.prim.req-static-assert-print.cpp`).
- Non-`bool` atom diagnosed where written (`check_constraint_expression`
  in concepts/mod.rs, called by `act_concept` and the requires-clause
  primary parser); the concept/clause becomes a RecoveryExpr, naming the
  concept yields an error expression (no further diag), a clause's atom
  is silently unsatisfied. Divergence: clang drops the whole declaration
  ("use of undeclared identifier 'f'"); kiln keeps it and reports
  "no matching function". Test `temp.constr.atomic-non-bool-at-definition.cpp`.
- `note_ambiguous_atomic_constraints` (+ new
  `note_ambiguous_atomic_constraints_similar_expression`) after an
  ambiguous call: `ambiguous_constraint_notes` in concepts/subsume.rs,
  called from `with_ovl_notes` (overload/diag.rs). `exprs_equivalent`
  (overload/redecl.rs) now compares `sizeof(T)`/type-trait operands by
  canonical type, so such redeclarations are redefinitions like clang.
  Tests `temp.constr.order-ambiguous-atomic-note.cpp`,
  `temp.over.link-requires-clause-trait-equivalence.cpp`.
- Item 4 DONE.

## Item 3 — atom_cache: DONE (hits only for multi-level constraints)
- `ConceptState::atom_cache: (ExprId, Vec<canonical ArgListId>) ->
  (bool, records)` in concepts/mod.rs; `sat_atom` (concepts/sat.rs)
  wraps `sat_atom_uncached`. Not cached when levels are dependent or the
  evaluation emitted/trapped an error. Sound because an atom ExprId
  belongs to one entity's constraint and the key is that entity's levels.
- Counters `atoms`/`atom_hits` in `SatCounters`; the report totals row
  gained `"sat","sat_cached","atoms","atoms_cached"`, text format a
  `satisfaction: N (M cached)   atoms: A (H cached)` line.
- Test `templates/report/temp.inst-report-atom-cache.cpp` (atoms_cached
  is 0 there: single-level checks are already served by `sat_cache`).
- Member templates of class template specializations used to drop
  their requires-clause; fixed (templ/member.rs `param_list` keeps the
  unsubstituted requires-clause and type-constraints, like clang).

## Item 5 — trailing requires on members of class templates: DONE
- `member_constraints` (pattern constraints + specialization levels),
  checked in `check_function_constraints`; "more constrained" tie-break
  (`order_constrained_candidates` / `same_param_type_lists`); uses of an
  unsatisfied member diagnosed (`diagnose_unsatisfied_use`).
- Hidden friend definitions in class templates now instantiate
  (subst/decls.rs `function_decl` links the friend to its pattern).
- Tests `over.match.best-more-constrained-member.cpp`,
  `over.match.viable-member-trailing-requires.cpp`.

## Item 6 — clang-ast accepts non-dependent RequiresExpr: DONE
- Dump prints `RequiresExpr 'bool' satisfied|unsatisfied`; normalizer
  drops per-requirement `satisfied`; TypeRequirement `'q':'desugared'`;
  MemberExpr `non_odr_use_unevaluated` (kiln + normalizer).
- Golden `templates/concepts/expr.prim.req-ast.cpp` restored.

## Item 7 — spec/status: DONE
- spec/sema-templates.md §11.2 "Landed (WP-109)"; spec/ast-dump.md
  RequiresExpr/MemberExpr; docs/status/libcxx-syntax.md rows refreshed
  (type_traits 0, compare 48, utility 81, array 177, span 113,
  string_view 329).
