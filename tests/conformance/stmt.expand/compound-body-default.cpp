// [stmt.expand]/1 (P1306R5): a body that is not a compound-statement is
// an extension diagnosed only on request (-Wexpansion-stmt-missing-braces
// or -pedantic); by default it is silent (clang 23, GCC 16.1). See
// compound-body.cpp, compound-body-pedantic.cpp.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
int f() {
  int s = 0;
  template for (auto x : {1, 2}) s += x;
  template for (auto x : {1, 2}) [[]] { s += x; }
  return s;
}
