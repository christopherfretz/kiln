// [expr.const]/5, [c.math.abs], [c.math.syn]: clang 19 does not evaluate
// abs, the rounding builtins, fma, sqrt and the other libm functions, nor
// trap/abort; a call is "not valid in a constant expression" at the call.
// fpclassify needs six arguments, the first five convertible to int.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:19 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 28:20 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 29:22 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 30:22 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 31:21 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 32:27 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 34:15 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 33:43 note "subexpression not valid in a constant expression"
//@ diag: 34:19 note "in call to 'g(0)'"
//@ diag: 35:22 error "constexpr variable 'i' must be initialized by a constant expression"
//@ diag: 36:15 error "static assertion failed due to requirement 'int(__builtin_fmax(1, 3)) == 1'"
//@ diag: 36:41 note "expression evaluates to '3 == 1'"
//@ diag: 37:15 error "static assertion failed due to requirement '__builtin_isinf_sign(-__builtin_inf()) == 1'"
//@ diag: 37:54 note "expression evaluates to '-1 == 1'"
//@ diag: 38:15 error "static assertion failed due to requirement '__builtin_fpclassify(0, 1, 2, 3, 4, __builtin_inff()) == 3'"
//@ diag: 38:69 note "expression evaluates to '1 == 3'"
//@ diag: 39:45 error "too few arguments to function call, expected 6, have 5"
//@ diag: 40:39 error "passing 'const char[2]' to parameter of incompatible type 'int'"
//@ diag: 41:45 error "floating point classification requires argument of floating point type (passed in 'int')"
//@ diag: 42:55 error "constexpr variable 'm' must be initialized by a constant expression"
constexpr int a = __builtin_abs(-1);
constexpr long b = __builtin_labs(-1);
constexpr double c = __builtin_sqrt(4.0);
constexpr double d = __builtin_floor(1.5);
constexpr float e = __builtin_fmaf(1, 2, 3);
constexpr long double f = __builtin_fmodl(5, 3);
constexpr int g(int n) { if (n) return n; __builtin_trap(); }
constexpr int h = g(0);
constexpr double i = __builtin_ldexp(1.0, 2) + __builtin_rint(0.5);
static_assert(int(__builtin_fmax(1, 3)) == 1);
static_assert(__builtin_isinf_sign(-__builtin_inf()) == 1);
static_assert(__builtin_fpclassify(0, 1, 2, 3, 4, __builtin_inff()) == 3);
int j = __builtin_fpclassify(0, 1, 2, 3, 1.0);
int k = __builtin_fpclassify(0, 1, 2, "x", 4, 1.0);
int l = __builtin_fpclassify(0, 1, 2, 3, 4, 5);
constexpr int m = __builtin_fpclassify(0, 1, 2, 3, 4, __builtin_sqrt(2.0));
