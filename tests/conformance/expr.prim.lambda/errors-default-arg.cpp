// [dcl.fct.default]/4: a lambda's parameters follow the default argument rules.
//@ xfail(gcc): GCC 16.1 accepts a lambda parameter without a default argument after one with one ([dcl.fct.default]/4)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:30 error "missing default argument on parameter 'y'"
void f() {
  auto g = [](int x = 1, int y) { return x + y; };
}
