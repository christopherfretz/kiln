// [expr.prim.lambda.closure]/5, [dcl.fct]/6 (C++23, P0847R7): a lambda may
// declare an explicit object parameter, enabling recursive lambdas.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(kiln): explicit object parameters in a lambda are not supported yet
constexpr int fib_test() {
  auto fib = [](this auto const& self, int n) -> int {
    return n < 2 ? n : self(n - 1) + self(n - 2);
  };
  return fib(10);
}
static_assert(fib_test() == 55);
