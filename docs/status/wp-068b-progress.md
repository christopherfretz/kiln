# WP-068b progress — operator overloading

Branch `wp/068b-operators`, worktree `kiln/.worktrees/wp-068b`.

## Done
- lookup.rs: DeclName-keyed variants (`lookup_in_dn`, `lookup_qualified_dn`,
  `lookup_name_dn`, `lookup_in_class_dn`, `current_scope_raw_dn`).
- decl.rs `act_on_function`: redeclaration lookup keyed by DeclName so
  operator functions (op_name form) participate; only replaceable global
  new/delete keeps the special path.

## In progress
- Declarations: normalize `SpecialName::Operator` declarators into the
  op_name form; `check_operator_decl` (§6.1).

## Next
1. `overload/oper.rs` resolve_operator + CXXOperatorCallExpr.
2. `overload/builtin.rs` built-in candidates.
3. Rewritten comparisons.
4. `->` chains, call-on-object, surrogates; `alloc.rs`.
5. Tests, xfail flips, bench, spec §10, type_traits probe.
