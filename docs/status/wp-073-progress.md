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

- Constraint checks in function deduction (`deduce_call.rs`, Tdk
  ConstraintsNotSatisfied + note chain), class/var template selection
  (`ensure_template_constraints`), partial specs (`deduce_partial` +
  `find_partial_decl` distinguishing same-args partials by printed head).
- Eager evaluation of non-dependent requires-expressions (`act_requires_expr`).
- Subsumption: `normal.rs` hash-consed normal forms, `subsume.rs` DNF/CNF
  (cap 65,536 clauses), tie-break in `order.rs` (functions + partials).

## Not yet
- Constrained auto / abbreviated templates.
- Tests in `tests/conformance/templates/concepts/`.

## Known divergences
- Type-requirement failure note is placed at the requires keyword (no type loc).
- Member-access failure note at member name, clang at `.`.

## Next step
See "Not yet", in order.
