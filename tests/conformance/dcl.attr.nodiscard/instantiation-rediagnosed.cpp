// [dcl.attr.nodiscard], [stmt.expr] (WP-217): an instantiation rebuilds
// every expression statement and for-increment (clang TransformExprStmt
// -> ActOnExprStmt), so the unused-result and unused-value warnings of a
// non-dependent one are reported again per instantiation with its notes,
// not deduplicated against the template definition's. The comma's left
// operand is checked only when the comma is built; a deprecated use is
// not re-diagnosed.
//@ strict-diags
//@ diag: 62:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 63:3 warning "ignoring return value of function declared with 'nodiscard' attribute: why"
//@ diag: 64:3 warning "ignoring return value of function declared with pure attribute"
//@ diag: 65:3 warning "declared with 'nodiscard' attribute"
//@ diag: 66:5 warning "equality comparison result unused"
//@ diag: 66:5 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 68:12 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 70:3 warning "expression result unused; should this cast be to 'void'?"
//@ diag: 71:9 warning "'d' is deprecated"
//@ diag: 54:3 note "'d' has been explicitly marked deprecated here"
//@ diag: 72:10 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 67:5 warning "expression result unused"
//@ diag: 69:3 warning "left operand of comma operator has no effect"
//@ diag: 69:6 warning "expression result unused"
//@ diag: 62:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 74:15 note "in instantiation of function template specialization 't<int>' requested here"
//@ diag: 63:3 warning "ignoring return value of function declared with 'nodiscard' attribute: why"
//@ diag: 64:3 warning "ignoring return value of function declared with pure attribute"
//@ diag: 65:3 warning "declared with 'nodiscard' attribute"
//@ diag: 66:5 warning "equality comparison result unused"
//@ diag: 66:5 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 68:12 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 70:3 warning "expression result unused; should this cast be to 'void'?"
//@ diag: 72:10 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 67:5 warning "expression result unused"
//@ diag: 69:6 warning "expression result unused"
//@ diag: 76:28 warning "equality comparison result unused"
//@ diag: 76:28 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 77:5 warning "equality comparison result unused"
//@ diag: 78:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 76:28 warning "equality comparison result unused"
//@ diag: 80:15 note "in instantiation of function template specialization 'u<1>' requested here"
//@ diag: 76:28 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 77:5 warning "equality comparison result unused"
//@ diag: 78:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 76:28 warning "equality comparison result unused"
//@ diag: 81:15 note "in instantiation of function template specialization 'u<2>' requested here"
//@ diag: 76:28 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 77:5 warning "equality comparison result unused"
//@ diag: 78:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 82:42 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 82:42 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 83:17 note "in instantiation of member function 'S<int>::m' requested here"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
[[deprecated]] int d();
[[nodiscard]] int f();
[[nodiscard("why")]] int fm();
int p() __attribute__((pure));
struct [[nodiscard]] R {};
R mk();
int g;
template <class T> void t(T x) {
  f();
  fm();
  p();
  mk();
  g == 1;
  g + 1;
  (void)0, f();
  1, g;
  (void*)&g;
  (void)d();
  if (x) f();
}
template void t<int>(int);
template <int N> void u() {
  for (int i = 0; i < 2; i == 1) {}
  N == 1;
  f();
}
template void u<1>();
template void u<2>();
template <class T> struct S { void m() { f(); } };
template struct S<int>;
