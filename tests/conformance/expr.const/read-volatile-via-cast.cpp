// [expr.const]/10.9: an lvalue-to-rvalue conversion of a volatile object is
// not a core constant expression, even through const_cast. Found by the
// clang negative-test run (WP-912).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 10:15 error "constexpr variable 'm' must be initialized by a constant expression"
//@ diag: 10:19 note "read of volatile object 'n1' is not allowed in a constant expression"
//@ diag: 9:24 note "volatile object declared here"
volatile constexpr int n1 = 0;
constexpr int m = const_cast<const int &>(n1);
