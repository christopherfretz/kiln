# WP-068b progress — operator overloading

Branch `wp/068b-operators`, worktree `kiln/.worktrees/wp-068b`.

## Done
- lookup.rs: DeclName-keyed variants (`lookup_in_dn`, `lookup_qualified_dn`,
  `lookup_name_dn`, `lookup_in_class_dn`, `current_scope_raw_dn`).
- decl.rs `act_on_function`: redeclaration lookup keyed by DeclName so
  operator functions (op_name form) participate; only replaceable global
  new/delete keeps the special path.

- Declarations: operator declarators normalized to op_name form;
  `check_operator_decl` (§6.1) with diag ids.
- AST: `CXXOperatorCallExpr` / `CXXRewrittenBinaryOperator` builder + dump;
  sema helper sites (begin_loc, unused-value/comparison, except, noreturn,
  side effects), Oven (`method_call` split, operator calls dispatch as
  calls/member calls, rewritten recurse to semantic form), codegen
  (`emit_method_call` split, operator calls in scalar/lvalue/agg paths).

## In progress
- `overload/oper.rs` resolve_operator (member/non-member/ADL candidates)
  and dispatch from act_on_binary/unary/subscript/call.

## Next
1. `overload/oper.rs` resolve_operator + CXXOperatorCallExpr.
2. `overload/builtin.rs` built-in candidates.
3. Rewritten comparisons.
4. `->` chains, call-on-object, surrogates; `alloc.rs`.
5. Tests, xfail flips, bench, spec §10, type_traits probe.
