// clang's `annotate` on a statement, clang 19 (WP-954): `cannot be applied
// to a statement`; clang 22 accepts it (annotate-statement-run.cpp).
// [dcl.attr.grammar]/5, [stmt.pre]/1.
//@ requires: clang<22
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:5 error "'annotate' attribute cannot be applied to a statement"
//@ diag: 13:5 error "'annotate' attribute cannot be applied to a statement"
int f(int n) {
  int s = 0;
  [[clang::annotate("loop", 1)]] for (int i = 0; i < n; ++i) s += i;
  [[clang::annotate("ret")]] return s;
}
int main() { return f(4) == 6 ? 0 : 1; }
