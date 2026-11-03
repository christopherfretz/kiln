// [stmt.expand]/1 (P1306R5): the body of an expansion statement is a
// compound-statement; a bare statement is an error.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 9:34 error "expected '{'"
int f() {
  int s = 0;
  template for (auto x : {1, 2}) s += x;
  return s;
}
