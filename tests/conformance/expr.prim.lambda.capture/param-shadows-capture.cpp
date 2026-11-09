// [expr.prim.lambda.capture]/5: an identifier in a simple-capture shall not
// be the name of a parameter of the lambda. Found by the clang negative-test
// run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, lambda parameters shadowing explicit captures are accepted
//@ expect: compile-fail
//@ diag: 10:17 error "a lambda parameter cannot shadow an explicitly captured entity"
void f() {
  int x = 0;
  (void)[x](int x) { return x; };
}
