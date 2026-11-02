// [c.math.syn] (P0533), [expr.const]/5, C23 F.10, [library.c]/3: a
// constexpr math call is not a constant expression when it raises an IEEE
// exception other than inexact: invalid (a signaling NaN operand, an
// out-of-range lround/llrint, fmod/remainder by zero or of an infinity,
// inf * 0 in fma, ilogb of zero or NaN), overflow (fma, fdim, ldexp,
// nextafter past the largest finite value), divide-by-zero (logb of zero),
// or underflow (an inexact ldexp into the subnormals, and every nextafter /
// nexttoward / nextafterf16 result that is subnormal or zero, which C23
// F.10.8.3 has raise underflow); nor is integer abs of the most negative
// value ([c.math.abs]: undefined). GCC 14 rejects the same calls (it folds
// llabs(LLONG_MIN) with a wrapped value and frexp of a signaling NaN).
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ skip(clang): clang 19/22 constant-evaluate none of these builtins
//@ diag: 47:22 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 48:20 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 49:25 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 50:22 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 51:22 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 52:21 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 54:15 error "constexpr variable 'g' must be initialized by a constant expression"
//@ diag: 53:52 note "subexpression not valid in a constant expression"
//@ diag: 54:19 note "in call to 'quo(inf, 2.000000e+00)'"
//@ diag: 55:22 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 56:22 error "constexpr variable 'i' must be initialized by a constant expression"
//@ diag: 57:22 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 58:22 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 59:19 error "constexpr variable 'l' must be initialized by a constant expression"
//@ diag: 60:22 error "constexpr variable 'm' must be initialized by a constant expression"
//@ diag: 61:19 error "constexpr variable 'n' must be initialized by a constant expression"
//@ diag: 62:22 error "constexpr variable 'o' must be initialized by a constant expression"
//@ diag: 63:22 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 64:22 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 65:24 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 66:24 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 67:24 error "constexpr variable 't' must be initialized by a constant expression"
//@ diag: 68:24 error "constexpr variable 'u' must be initialized by a constant expression"
//@ diag: 69:22 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 70:25 error "constexpr variable 'w' must be initialized by a constant expression"
//@ diag: 72:15 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 71:44 note "subexpression not valid in a constant expression"
//@ diag: 72:19 note "in call to 'fexp(nan)'"
//@ diag: 73:25 error "constexpr variable 'y' must be initialized by a constant expression"
//@ diag: 74:27 error "constexpr variable 'z' must be initialized by a constant expression"
constexpr double snan = __builtin_nans(""), inf = __builtin_inf(), big = __DBL_MAX__;
constexpr double a = __builtin_ceil(snan);
constexpr long b = __builtin_lround(1e300);
constexpr long long c = __builtin_llrint(__builtin_nan(""));
constexpr double d = __builtin_fmod(1.0, 0.0);
constexpr double e = __builtin_fmod(inf, 1.0);
constexpr float f = __builtin_remainderf(1.0f, 0.0f);
constexpr int quo(double x, double y) { int q = 0; __builtin_remquo(x, y, &q); return q; }
constexpr int g = quo(inf, 2.0);
constexpr double h = __builtin_fma(big, 2.0, 0.0);
constexpr double i = __builtin_fdim(big, -big);
constexpr double j = __builtin_ldexp(1.0, 1024);
constexpr double k = __builtin_ldexp(1.5, -1074);
constexpr int l = __builtin_ilogb(0.0);
constexpr double m = __builtin_logb(-0.0);
constexpr int n = __builtin_ilogbf(__builtin_nanf(""));
constexpr double o = __builtin_nextafter(big, inf);
constexpr double p = __builtin_nextafter(0x1p-1022, 0.0);
constexpr double q = __builtin_nexttoward(0x1p-1074, 0.0L);
constexpr _Float16 r = __builtin_nextafterf16(0.0f16, 1.0f16);
constexpr _Float16 s = __builtin_nextafterf16(0x1p-14f16, 0.0f16);
constexpr _Float16 t = __builtin_nextafterf16(0x1.ffcp15f16, __builtin_inff16());
constexpr _Float16 u = __builtin_nextafterf16(__builtin_nansf16(""), 1.0f16);
constexpr __bf16 v = __builtin_nextafterf16b(0.0bf16, -1.0bf16);
constexpr long long w = __builtin_llabs(-__LONG_LONG_MAX__ - 1);
constexpr int fexp(double x) { int ex = 0; __builtin_frexp(x, &ex); return ex; }
constexpr int x = fexp(snan);
constexpr _Float128 y = __builtin_fmaf128(0.0f128, __builtin_inff128(), 1.0f128);
constexpr long double z = __builtin_nextafterl(__LDBL_MAX__, __builtin_infl());
