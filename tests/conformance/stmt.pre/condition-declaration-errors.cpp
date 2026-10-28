// [stmt.pre]/1: a condition declaration has a brace-or-equal-initializer
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:11 error "variable declaration in condition must have an initializer"
//@ diag: 8:14 error "variable declaration in condition cannot have a parenthesized initializer"
void f() {
  if (int x) {}
  while (int y(1)) {}
}
