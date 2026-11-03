// [expr.const]: clang 19 constant-evaluates neither
// __builtin_elementwise_add_sat nor _sub_sat: a call is not a constant
// expression ("subexpression not valid" inside a constexpr function, no
// note at top level). elementwise-sat-constexpr-clang22.cpp has clang 22
// (WP-432).
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:19 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:33 note "subexpression not valid in a constant expression"
//@ diag: 19:15 note "in call to 'f(3)'"
static_assert(__builtin_elementwise_add_sat(2147483647, 1) == 2147483647);
static_assert(__builtin_elementwise_sub_sat(3u, 4u) == 0u);
constexpr int k = __builtin_elementwise_add_sat(1, 2);
constexpr int f(int a) { return __builtin_elementwise_sub_sat(a, 1); }
static_assert(f(3) == 2);
// Not a constant expression, so evaluated at run time.
int g(int a) { return __builtin_elementwise_add_sat(a, 1); }
