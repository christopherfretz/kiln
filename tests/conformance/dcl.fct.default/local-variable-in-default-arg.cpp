// [dcl.fct.default]/7: a local variable shall not appear as a potentially-
// evaluated expression in a default argument. Found by the clang negative-
// test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 9:25 error "default argument references local variable 'i' of enclosing function"
void g() {
  int i = 0;
  extern void h(int x = i);
}
