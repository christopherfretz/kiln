# WP-138 progress: `<optional>` / `<tuple>` blockers

Branch `wp/138-optional-tuple-blockers`, worktree `kiln/.worktrees/wp-138`.

## Done
- [temp.over.link]/7: special-member template heads are compared, so
  constructor templates that differ only in an `enable_if` NTTP are
  distinct (`class/special/declare.rs`).
- `decltype(a > b)` in template arguments: verified fixed on main.
- [temp.dep.expr]/3: implicit-`this` calls to overloaded members of a
  class template are deferred, including `S::f()`, whose
  injected-class-name maps to the instantiation (`expr.rs`,
  `templ/subst/exprs.rs` `member_nns`).
- Partially substituted pack expansions (`Conv<U, T>...` in a member
  template), handled in keep mode with `SubstTemplateTypeParmPack`
  (`templ/pack.rs`, `templ/subst/types.rs`). Type packs only.
- Deduction: a trailing function parameter pack after a pack deduced
  from a type, and the trailing pack's explicit prefix
  (`templ/deduce_call.rs`).
- `__reference_binds_to_temporary`, `__reference_constructs_from_temporary`
  and `__reference_converts_from_temporary` (`templ/builtins/traits.rs`).
- Member class template partial specializations are instantiated with
  their enclosing class template (`templ/member.rs`
  `member_partial_spec`, `templ/class.rs`). Converted argument packs are
  substituted (`templ/subst/mod.rs`).
- A value-dependent `explicit(expr)` is substituted per specialization
  and per class instantiation (`templ/funcs.rs`,
  `templ/subst/decls.rs`, `overload/ics.rs`).

## Results
- Header-only TUs:
  - `<optional>`: 3 → 0
  - `<tuple>`: 0 → 0
  - `<memory>`: 6 → 3 (`__atomic_*`, WP-144)
  - `<variant>`: 20 → 20 (`using B::operator=`)
  - `<vector>`: 50 → 48
- A libc++ tuple probe (`tie`, converting constructors, `make_tuple`,
  `tuple_cat`, `common_reference`) now compiles with 0 errors.

## Not done / follow-ups
- [over.match.ref]: an explicit conversion function in a reference's
  direct-initialization (`int&& r(Ex{})`) gives "no viable conversion".
- Inheriting `using B::operator=` (all 20 `<variant>` errors).
- Partial substitution of non-type and function parameter packs (keep
  mode).
- Out-of-class partial specializations of member templates
  (`template<class T> template<class U> struct O<T>::I<U*>`).
- The backlog row's `sizeof...(T) >= 1` and mem-initializer pack items
  (`tuple:587`, `tuple:616`) compile in the tuple probe after the fixes
  above. No separate change was needed.
