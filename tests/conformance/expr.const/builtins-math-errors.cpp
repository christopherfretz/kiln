// [expr.const]/5, [c.math.abs], [c.math.syn], [library.c]/3: cbrt and the
// other non-P0533 libm functions (sqrt folds since WP-393) and trap/abort are never constants; the
// P0533 set (abs, floor, fma, fmod, ilogb, ...) is not when the call is
// undefined or raises an IEEE exception other than inexact; a call is "not
// valid in a constant expression" at the call, except abs of the most
// negative value, which fails without a note (the error is at the variable,
// as clang 22, which evaluates abs; clang 19 evaluates none of them).
// fpclassify needs six arguments, the first five convertible to int.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 32:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 33:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 34:22 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 35:22 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 36:21 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 37:27 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 39:15 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 38:43 note "subexpression not valid in a constant expression"
//@ diag: 39:19 note "in call to 'g(0)'"
//@ diag: 40:22 error "constexpr variable 'i' must be initialized by a constant expression"
//@ diag: 41:15 error "static assertion failed due to requirement 'int(__builtin_fmax(1, 3)) == 1'"
//@ diag: 41:41 note "expression evaluates to '3 == 1'"
//@ diag: 42:15 error "static assertion failed due to requirement '__builtin_isinf_sign(-__builtin_inf()) == 1'"
//@ diag: 42:54 note "expression evaluates to '-1 == 1'"
//@ diag: 43:15 error "static assertion failed due to requirement '__builtin_fpclassify(0, 1, 2, 3, 4, __builtin_inff()) == 3'"
//@ diag: 43:69 note "expression evaluates to '1 == 3'"
//@ diag: 44:45 error "too few arguments to function call, expected 6, have 5"
//@ diag: 45:39 error "passing 'const char[2]' to parameter of incompatible type 'int'"
//@ diag: 46:45 error "floating point classification requires argument of floating point type (passed in 'int')"
//@ diag: 47:55 error "constexpr variable 'm' must be initialized by a constant expression"
constexpr int a = __builtin_abs(-__INT_MAX__ - 1);
constexpr long b = __builtin_labs(-__LONG_MAX__ - 1);
constexpr double c = __builtin_cbrt(8.0);
constexpr double d = __builtin_floor(__builtin_nans(""));
constexpr float e = __builtin_fmaf(__builtin_inff(), 0, 1);
constexpr long double f = __builtin_fmodl(5, 0);
constexpr int g(int n) { if (n) return n; __builtin_trap(); }
constexpr int h = g(0);
constexpr double i = __builtin_ilogb(0.0) + __builtin_ldexp(1.0, 2);
static_assert(int(__builtin_fmax(1, 3)) == 1);
static_assert(__builtin_isinf_sign(-__builtin_inf()) == 1);
static_assert(__builtin_fpclassify(0, 1, 2, 3, 4, __builtin_inff()) == 3);
int j = __builtin_fpclassify(0, 1, 2, 3, 1.0);
int k = __builtin_fpclassify(0, 1, 2, "x", 4, 1.0);
int l = __builtin_fpclassify(0, 1, 2, 3, 4, 5);
constexpr int m = __builtin_fpclassify(0, 1, 2, 3, 4, __builtin_cbrt(2.0));
