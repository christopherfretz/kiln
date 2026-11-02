// [stmt.expr]/1, [expr.sub], [expr.call], [expr.prim.lambda.closure]/16,
// [expr.prim.id.qual]/3: where clang's delayed typo correction reports a
// qualified name that lookup does not find (`S::nom`). A subscript missing
// its `]` drops its index, whose typo is then only reported at the end of
// the translation unit (after the pending instantiations); the base of a
// subscript is corrected at the `]`; a call missing its `)` corrects its
// callee and arguments at once (no "expected ')'" when one held a typo);
// the statements and declarations of a lambda body or a statement
// expression end their own full-expressions. The order is checked by the
// kiln_parse test `delayed_typo_correction_points`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:19 error "expected ']'"
//@ diag: 37:10 note "to match this '['"
//@ diag: 38:23 error "expected ']'"
//@ diag: 38:14 note "to match this '['"
//@ diag: 39:10 error "no member named 'nom3' in 'S'"
//@ diag: 39:17 error "expected ']'"
//@ diag: 39:14 note "to match this '['"
//@ diag: 39:22 error "expected expression"
//@ diag: 40:6 error "no member named 'nom4' in 'S'"
//@ diag: 41:12 error "no member named 'nom5' in 'S'"
//@ diag: 41:22 error "expected expression"
//@ diag: 42:19 error "no member named 'nom6' in 'S'"
//@ diag: 42:41 error "expected expression"
//@ diag: 43:17 error "no member named 'nom7' in 'S'"
//@ diag: 43:34 error "expected expression"
//@ diag: 35:36 error "member reference base type 'int' is not a structure or union"
//@ diag: 44:3 note "in instantiation of function template specialization 'tf<int>' requested here"
//@ diag: 37:14 error "no member named 'nom1' in 'S'"
//@ diag: 38:18 error "no member named 'nom2' in 'S'"
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
}
