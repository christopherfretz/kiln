# WP-073 progress (concepts)

Branch `wp/073-concepts`, worktree `.worktrees/wp-073`.

## Works
- `templ/concepts/{mod,sat,requires,normal}.rs`: concept-id satisfaction
  (memoized on (concept, canonical args), counters `sat.count`/`cache_hits`),
  expression-based &&/|| satisfaction, atoms substituted under a SFINAE
  `ConstraintSubstitution` frame, non-bool atom error, §8.7 note chains
  (single-arg "does not satisfy", elaborated comparisons, nested chains).
- Concept-ids are checked when formed non-dependent (`build_concept_id`) and
  when substituted (`Substituter::concept_id_expr`); `ExprNode.aux` holds the
  result (`ExprArena::satisfied`), the Oven folds it.
- Requires-expressions substituted (`Substituter::requires_expr`): params,
  simple/type/compound(noexcept, return-type-requirement)/nested requirements.
- static_assert of a failing concept-id/requires-expr prints clang's chain.

## Not yet
- Constraint checks in function deduction (`deduce_spec`), class template
  selection (`get_class_spec`), partial specs (`deduce_partial`), spec.rs stub.
- Eager evaluation of non-dependent requires-expressions (`act_requires_expr`).
- Subsumption (`subsume.rs`), normal.rs normalization, overload tie-breaker.
- Constrained auto / abbreviated templates.
- Tests in `tests/conformance/templates/concepts/`.

## Known divergences
- Type-requirement failure note is placed at the requires keyword (no type loc).
- Member-access failure note at member name, clang at `.`.

## Next step
See "Not yet", in order.
