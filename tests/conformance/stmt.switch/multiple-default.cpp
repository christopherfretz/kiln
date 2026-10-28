// [stmt.switch]/2: at most one default label
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:3 error "multiple default labels in one switch"
//@ diag: 8:3 note "previous case defined here"
void f(int x) {
  switch (x) {
  default:
  default:
    break;
  }
}
