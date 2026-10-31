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

- `->` chains (`overload/arrow.rs`): cycle detection, depth limit 256,
  notes as clang. Surrogate call candidates (`surrogate.rs`).
- `overload/alloc.rs`: class-scope `operator new`/`delete` lookup
  ([expr.new]/12, [expr.delete]/9), usual deallocation selection, "no
  suitable member" diag; member operator delete implicitly noexcept.
- Oven: reference-returning class calls never use sret.
- Conformance tests: over.ref, over.call.object, over.match.oper,
  class.compare, over.built (incl. 144/121-note goldens), over.oper,
  expr.new, expr.delete; AST goldens (xtask normalizer now handles
  CXXOperatorCallExpr / CXXRewrittenBinaryOperator). Full conformance:
  1154 pass / 0 fail / 12 xfail / 0 xpass.
- Static operator() / [] with a prvalue object: materialized (as clang).

- Clippy clean; bench `bench/overload/{arrow-chain,less-10k}.cpp`
  (release kiln 0.37 s / 76 MB and 1.41 s / 17 MB; clang 1.06 s / 297 MB
  and 2.25 s / 97 MB; host load avg ~6 on 24 cores, so no baseline
  recorded). Spec sema-overload §10.6. type_traits probe: 24 errors
  (main 36; all 12 "overloaded operators is not supported yet" gone).

## State
Full `cargo xtask ci` green (20:58 EDT): clang self-check 1145 pass /
3 xfail / 23 skipped; kiln 1159 pass / 0 fail / 12 xfail / 0 xpass of
1171. Ready for review; nothing pending in this WP.
