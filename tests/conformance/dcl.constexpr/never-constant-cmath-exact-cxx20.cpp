// [dcl.constexpr]/6 (before C++23, IFNDR): a constexpr function calling a
// <cmath> builtin that kiln's Oven evaluates exactly (P0533's rounding,
// remainder and exponent families, `fma`, `sqrt`) may produce a constant,
// so it is not reported as never constant (GCC agrees; clang 22 does not
// evaluate these builtins and reports the function); a transcendental
// builtin still is (spec/sema.md "Never constant"). Found running GCC's
// C++ testsuite (WP-1468).
//@ std: c++20
//@ requires: clang>=23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:18 error "constexpr function never produces a constant expression"
//@ diag: 18:40 note "subexpression not valid in a constant expression"
constexpr float r1(float x) { return __builtin_sqrtf(x); }
constexpr double r2(double x) { return __builtin_floor(x) + __builtin_fma(x, x, 1.0); }
constexpr long double r3(long double x) { return __builtin_truncl(x); }
static_assert(r1(4.0f) == 2.0f && r2(1.5) == 4.25 && r3(2.5L) == 2.0L);
constexpr double t1(double x) { return __builtin_sin(x); }
