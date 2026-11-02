// [stmt.expr]/1, [expr.sub], [expr.call], [expr.prim.lambda.closure]/16,
// [expr.prim.id.qual]/3: where clang's delayed typo correction reports a
// qualified name that lookup does not find (`S::nom`). A subscript missing
// its `]` drops its index, whose typo is then only reported at the end of
// the translation unit (after the pending instantiations); the base of a
// subscript is corrected at the `]`; a call missing its `)` corrects its
// callee and arguments at once (no "expected ')'" when one held a typo) and
// makes the expression invalid, so an expression statement missing its `;`
// after it skips to the next `;` with no "expected ';'";
// the statements and declarations of a lambda body or a statement
// expression end their own full-expressions. The order is checked by the
// kiln_parse test `delayed_typo_correction_points`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:19 error "expected ']'"
//@ diag: 44:10 note "to match this '['"
//@ diag: 45:23 error "expected ']'"
//@ diag: 45:14 note "to match this '['"
//@ diag: 46:10 error "no member named 'nom3' in 'S'"
//@ diag: 46:17 error "expected ']'"
//@ diag: 46:14 note "to match this '['"
//@ diag: 46:22 error "expected expression"
//@ diag: 47:6 error "no member named 'nom4' in 'S'"
//@ diag: 48:12 error "no member named 'nom5' in 'S'"
//@ diag: 48:22 error "expected expression"
//@ diag: 49:19 error "no member named 'nom6' in 'S'"
//@ diag: 49:41 error "expected expression"
//@ diag: 50:17 error "no member named 'nom7' in 'S'"
//@ diag: 50:34 error "expected expression"
//@ diag: 52:12 error "no member named 'nom8' in 'S'"
//@ diag: 54:11 error "no member named 'nom9' in 'S'"
//@ diag: 55:8 error "expected ';' after expression"
//@ diag: 42:36 error "member reference base type 'int' is not a structure or union"
//@ diag: 51:3 note "in instantiation of function template specialization 'tf<int>' requested here"
//@ diag: 44:14 error "no member named 'nom1' in 'S'"
//@ diag: 45:18 error "no member named 'nom2' in 'S'"
//@ requires: clang<22
// clang 22 emits error: expected ';' after expression; WP-355 residual
struct S { static int m; };
int arr[3];
int g(int);
template <class T> void tf(T t) { t.zz; }
void h(int a) {
  a = arr[S::nom1 x];
  int v = arr[S::nom2 x];
  a = S::nom3[1 2] + ;
  S::nom4(1;
  a = g(S::nom5 x) + ;
  [] { int v = S::nom6; return; }(), 1 +;
  ({ int w = S::nom7; w; }) + 5 +;
  tf(1);
  a = g(S::nom8 x)
  a = 1;
  ({ g(S::nom9 z) 1; });
  a = 2
}
