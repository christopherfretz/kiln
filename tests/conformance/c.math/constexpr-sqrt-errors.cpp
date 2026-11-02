// [c.math.syn] (constexpr sqrt, P1383), [expr.const]/5, [library.c]/3,
// C23 F.10.4.5: sqrt of a value below -0 (a negative finite or -inf) or of
// a signaling NaN raises invalid, so the call is not a constant
// expression, in every format. g++ 14 rejects the same calls.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ skip(clang): clang 19/22 constant-evaluate no sqrt builtin
//@ diag: 21:22 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 22:22 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 23:22 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 24:22 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 25:21 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 26:27 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 27:27 error "constexpr variable 'g' must be initialized by a constant expression"
//@ diag: 28:25 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 29:22 error "constexpr variable 'i' must be initialized by a constant expression"
//@ diag: 31:18 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 30:42 note "subexpression not valid in a constant expression"
//@ diag: 31:22 note "in call to 'root(-4.000000e+00)'"
constexpr double a = __builtin_sqrt(-1.0);
constexpr double b = __builtin_sqrt(-0x1p-1074);
constexpr double c = __builtin_sqrt(-__builtin_inf());
constexpr double d = __builtin_sqrt(__builtin_nans(""));
constexpr float e = __builtin_sqrtf(-2.0f);
constexpr long double f = __builtin_sqrtl(-2.0L);
constexpr long double g = __builtin_sqrtl(__builtin_nansl(""));
constexpr _Float128 h = __builtin_sqrtf128(-1.0f128);
constexpr __fp16 i = __builtin_sqrtf16(-1.0f);
constexpr double root(double x) { return __builtin_sqrt(x); }
constexpr double j = root(-4.0);
