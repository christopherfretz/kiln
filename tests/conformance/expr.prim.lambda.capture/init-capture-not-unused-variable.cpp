// [expr.prim.lambda.capture]/6, [dcl.attr.unused] (WP-703): an
// init-capture's variable is referenced by its capture, so it is never
// `-Wunused-variable` (clang names an unused one only in
// -Wunused-lambda-capture), whether by copy or by reference, in a generic
// lambda or in a lambda of a template instantiated with it; one that is
// only assigned in the body is still `-Wunused-but-set-variable`. Reduced
// from taskflow v3.10.0's unittests/test_runtimes.cpp.
//@ args: -Wunused-variable -Wunused-but-set-variable
//@ strict-diags
//@ expect: compile-ok
//@ diag: 25:14 warning "variable 'y' set but not used"
//@ diag: 32:16 warning "unused variable 'unused'"
int g(int);

template <class T> int t(T v) {
  auto l = [w = v]() { return 1; };
  auto m = [&w = v]() mutable { return 2; };
  return l() + m();
}

int f() {
  int x = 0;
  auto l1 = [&r = x]() mutable { return 1; };
  auto l2 = [r = x, s = g(1)]() { return 2; };
  auto l3 = [y = x]() mutable { y = 1; };
  auto l4 = [&r = x, y = 2]() { return y; };
  auto gl = [y = x](auto) { return 3; };
  l3();
  return l1() + l2() + l4() + gl(0) + t(1);
}

void h() { int unused = 0; }
