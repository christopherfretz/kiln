// [expr.prim.lambda.capture]/5: an identifier in a simple-capture shall not
// be the name of a parameter of the lambda. Found by the clang negative-test
// run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 10:17 error "a lambda parameter cannot shadow an explicitly captured entity"
//@ diag: 10:10 note "variable 'x' is explicitly captured here"
void f() {
  int x = 0;
  (void)[x](int x) { return x; };
}
