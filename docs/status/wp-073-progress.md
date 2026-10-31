# WP-073 progress (concepts)

Branch `wp/073-concepts`, worktree `.worktrees/wp-073`.

## Status: feature-complete for the package; in the exit gate (full CI)

## Works
- `templ/concepts/{mod,sat,requires,normal,subsume}.rs`: satisfaction
  (memoized, counters `sat.count`/`sat.cache_hits`), requires-expressions,
  hash-consed normal forms, DNF/CNF subsumption (cap 65,536), tie-break.
- Call sites: function deduction, class/var template selection, partial
  specializations, static_assert note chains.
- Constrained `auto` variables and return types (`check_placeholder_constraint`),
  abbreviated templates, `template<C T>`, `template<C<U> T>`.
- Non-dependent requires-expressions evaluated at parse time.
- Unqualified type-requirements (`typename X<T>;`).
- 65 conformance tests in `tests/conformance/templates/concepts/` (all pass).
- `<type_traits>` probe: 20 -> 10 (4 packs/WP-072, 2 alias dependent-member,
  4 `__int128`).
- Spec: "Landed (WP-073)" note in `spec/sema-templates.md` §11.2.

## Not done (owners in the spec note)
- Packs in constraints (WP-072), generic lambdas, [over.match.best]/2.6,
  `atom_cache`, `note_ambiguous_atomic_constraints`.

## Next step
Full `cargo xtask ci` green, then hand back.
