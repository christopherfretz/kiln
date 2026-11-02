// [stmt.expr]/1, [expr.prim.id.qual]/3: a qualified name that lookup does
// not find (`S::nom`, `N::nom`) in an expression statement is reported at
// the end of the full-expression, as clang's delayed typo correction, so a
// missing `;` after it is reported first (the order is checked by the
// kiln_parse test `delayed_no_member_after_missing_semi`). A member access
// (`s.nomember`) and a name in a declaration's initializer, a condition or a
// return statement are reported where they are written.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:11 error "expected ';' after expression"
//@ diag: 25:6 error "no member named 'nom' in 'S'"
//@ diag: 26:11 error "expected ';' after expression"
//@ diag: 26:6 error "no member named 'nom' in namespace 'N'"
//@ diag: 27:14 error "no member named 'none' in 'S'"
//@ diag: 29:5 error "no member named 'nomember' in 'S'"
//@ diag: 31:15 error "no member named 'bad' in namespace 'N'"
//@ diag: 33:10 error "no member named 'nope' in namespace 'N'"
//@ diag: 34:13 error "no member named 'gone' in 'S'"
//@ diag: 34:19 error "expected ';' after return statement"
//@ requires: clang<22
// clang 22 no longer emits error: expected ';' after expression; WP-355 residual
struct S { int m; };
namespace N { int f(); }
void g(S s) {
  S::nom()
  N::nom()
  int a = S::none + 1
  ;
  s.nomember()
  ;
  N::f() + N::bad()
  ;
  if (N::nope()) {}
  return S::gone()
}
