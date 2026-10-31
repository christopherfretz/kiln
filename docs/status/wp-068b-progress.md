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

- `overload/oper.rs`: candidate set for operators (non-member from
  unqualified lookup + ADL with enum-only filter, members, reversed and
  rewritten candidates with P2468, template filter heuristic),
  `-Wambiguous-reversed-operator`, rewritten `==` non-bool check,
  `build_operator_call`, all outcome diagnostics. Compiles; not yet wired.
- `overload/builtin.rs`: [over.built] candidates in clang's builder
  order (type sets from conversion functions, VR quals, pointer/member
  pointer variants in clang CVR numbering, /3.3.4 via the enum pair rule),
  `sema.ovl.conv_types` cache + memory row, per-resolution ICS memo,
  `ovl.builtin_candidates`; `CandidateKind::Builtin`; notes
  (`built-in candidate operator<(int, int)` with C-policy printing,
  `note_ambiguous_type_conversion`); `OperResult::Converted` consumers;
  `builtin_subscript` + class-index subscript; display order now clang 19's
  `ConversionSignals`. Fix: conversion-function `nullptr_t` result is a
  null pointer constant. Scratch bi1/np match clang.

- Dispatch wired: act_on_binary -> binary_operator -> builtin_binary
  (BuiltinNotes via `trailing_notes`), act_on_unary -> builtin_unary,
  multi-arg subscript (`parse_expr_list`, `act_on_subscript_args`),
  call-on-object in act_on_call, braced assign rhs (forms.rs), subst arms
  for CXXOperatorCallExpr/CXXRewrittenBinaryOperator, ctorinit uninit arm.
  Display-order quirk (clang EffectiveFailureKind) via `note_extra_args`.
- Scratch cases match clang (n1, err, un, rw, amb; diffs only the
  pre-existing "use '=' to turn" note and WP-082 defaulted comparisons).
- libc++ probe: <type_traits> 24 errors (branch-before 27, main 36).

## Next
1. cargo test; fix kiln_parse/tests/class.rs operator asserts; ci --quick.
2. More builtin probes (++/--, =, compound, ->*, [] with class index).
3. `->` chains, call-on-object surrogates; `alloc.rs`.
4. Tests, xfail flips, bench, spec §10, type_traits probe.
