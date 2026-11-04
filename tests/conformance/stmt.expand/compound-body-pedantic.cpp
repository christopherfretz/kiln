// [stmt.expand]/1 (P1306R5): the non-compound-body extension is
// diagnosed under -pedantic, an error under -pedantic-errors (clang 23).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ args: -pedantic-errors
//@ diag: 10:34 error "ISO C++ requires the body of an expansion statement to be a compound statement"
int f() {
  int s = 0;
  template for (auto x : {1, 2}) s += x;
  return s;
}
