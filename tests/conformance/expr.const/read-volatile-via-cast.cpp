// [expr.const]/10.9: an lvalue-to-rvalue conversion of a volatile object is
// not a core constant expression, even through const_cast. Found by the
// clang negative-test run (WP-912).
//@ std: c++23
//@ xfail(kiln): WP-912 follow-up, the evaluator reads a volatile object through a cast
//@ expect: compile-fail
//@ diag: 9:15 error "constexpr variable 'm' must be initialized by a constant expression"
volatile constexpr int n1 = 0;
constexpr int m = const_cast<const int &>(n1);
