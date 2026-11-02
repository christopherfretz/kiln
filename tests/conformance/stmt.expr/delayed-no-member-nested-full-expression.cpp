// [stmt.expr], [expr.call], [expr.prim.id.qual]: a qualified name not found
// in its non-dependent class (`S::nom`) followed by another identifier
// inside a larger expression statement. clang keeps "no member named" as a
// delayed typo: inside call arguments it skips to the `)` and reports no
// "expected ')'"; in a parenthesized expression it reports "expected ')'"
// first (`(S::nom x)` is no cast); a subscript missing its `]` keeps the
// delayed typo until the end of the translation unit.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:8 error "no member named 'nom' in 'S'"
//@ diag: 38:11 error "no member named 'nom' in 'S'"
//@ diag: 39:12 error "no member named 'nom' in 'S'"
//@ diag: 40:8 error "no member named 'nom' in 'S'"
//@ diag: 41:11 error "no member named 'nom' in 'S'"
//@ diag: 42:11 error "expected ')'"
//@ diag: 42:3 note "to match this '('"
//@ diag: 42:7 error "no member named 'nom' in 'S'"
//@ diag: 43:15 error "expected ')'"
//@ diag: 43:7 note "to match this '('"
//@ diag: 43:11 error "no member named 'nom' in 'S'"
//@ diag: 44:12 error "expected ')'"
//@ diag: 44:4 note "to match this '('"
//@ diag: 44:8 error "no member named 'nom' in 'S'"
//@ diag: 45:18 error "expected ']'"
//@ diag: 45:10 note "to match this '['"
//@ diag: 46:14 error "no member named 'nom' in 'S'"
//@ diag: 46:17 error "expected ';' at end of declaration"
//@ diag: 47:12 error "no member named 'nom' in 'S'"
//@ diag: 45:14 error "no member named 'nom' in 'S'"
//@ requires: clang<22
// WP-399 (typo correction): clang 22 has no delayed typo correction, so its parser recovery after the unknown member differs.
struct S { static int m; };
void f(int);
void g(int, int);
int arr[3];
void h(int a) {
  f(S::nom x);
  g(1, S::nom x);
  a = f(S::nom x);
  f(S::nom x) + 1;
  S::m(S::nom x);
  (S::nom x);
  a = (S::nom x);
  ((S::nom x));
  a = arr[S::nom x];
  int v = S::nom x;
  a + f(S::nom x;
}
