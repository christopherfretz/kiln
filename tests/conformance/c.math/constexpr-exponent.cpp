// [c.math.syn] (constexpr frexp, ldexp, scalbn, scalbln, ilogb, logb, modf;
// P0533), C23 7.12.6 and F.10.3: the exponent functions are exact in every
// format: frexp gives a fraction in [0.5, 1) (zero, infinity and NaN as
// they are, exponent 0) and stores the exponent; ldexp/scalbn scale by a
// power of two, exactly into the subnormal range; ilogb/logb give the
// unbiased exponent (of a subnormal too); modf splits into fraction and
// integral part with x's sign. ilogb/logb of zero, an overflowing or
// inexactly underflowing ldexp are not constants (constexpr-errors.cpp).
// Expectations from g++ 14, which folds the same calls except ldexp of an
// infinity (its MPFR folder wants a finite input; exact under F.10.3.6).
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 constant-evaluate none of these builtins
constexpr bool neg(double x) { return __builtin_signbit(x); }

constexpr int fexp(double x) { int e = 99; __builtin_frexp(x, &e); return e; }
constexpr double ffrac(double x) { int e = 0; return __builtin_frexp(x, &e); }
static_assert(ffrac(8.0) == 0.5 && fexp(8.0) == 4);
static_assert(ffrac(-3.0) == -0.75 && fexp(-3.0) == 2);
static_assert(ffrac(0x1p-1074) == 0.5 && fexp(0x1p-1074) == -1073);
static_assert(ffrac(-0.0) == 0 && neg(ffrac(-0.0)) && fexp(0.0) == 0);
static_assert(ffrac(__builtin_inf()) == __builtin_inf());
constexpr int fexpf(float x) { int e = 0; __builtin_frexpf(x, &e); return e; }
constexpr int fexpl(long double x) { int e = 0; __builtin_frexpl(x, &e); return e; }
static_assert(fexpf(0x1p-149f) == -148 && fexpl(0.375L) == -1);

static_assert(__builtin_ldexp(1.0, 4) == 16.0 && __builtin_ldexp(3.0, -1) == 1.5);
static_assert(__builtin_ldexp(1.0, -1074) == 0x1p-1074 && __builtin_ldexp(0x1p-1074, 1074) == 1.0);
static_assert(__builtin_ldexp(0x1p-1022, 2045) == 0x1p1023);
static_assert(__builtin_ldexp(-0.0, 5) == 0 && neg(__builtin_ldexp(-0.0, 5)));
static_assert(__builtin_ldexp(__builtin_inf(), -5000) == __builtin_inf());
static_assert(__builtin_scalbn(1.0f, -1) == 0.5f && __builtin_scalbnl(1.5L, 3) == 12.0L);
static_assert(__builtin_scalbln(1.0, 1000L) == 0x1p1000 && __builtin_scalblnf(3.0f, -2L) == 0.75f);

static_assert(__builtin_ilogb(1024.0) == 10 && __builtin_ilogb(-0.75) == -1);
static_assert(__builtin_ilogb(0x1p-1074) == -1074 && __builtin_ilogbf(0x1p-149f) == -149);
static_assert(__builtin_ilogbl(3.0L) == 1);
static_assert(__builtin_logb(0.25) == -2.0 && __builtin_logbf(-48.0f) == 5.0f);
static_assert(__builtin_logb(0x1.8p-1070) == -1070 && __builtin_logbl(1.0L) == 0);
static_assert(__builtin_logb(-__builtin_inf()) == __builtin_inf());

constexpr double ipart(double x) { double i = 99; __builtin_modf(x, &i); return i; }
constexpr double fpart(double x) { double i = 0; return __builtin_modf(x, &i); }
static_assert(fpart(3.25) == 0.25 && ipart(3.25) == 3.0);
static_assert(fpart(-3.25) == -0.25 && ipart(-3.25) == -3.0);
static_assert(fpart(-4.0) == 0 && neg(fpart(-4.0)) && ipart(-4.0) == -4.0);
static_assert(fpart(-0.5) == -0.5 && ipart(-0.5) == 0 && neg(ipart(-0.5)));
static_assert(fpart(__builtin_inf()) == 0 && ipart(__builtin_inf()) == __builtin_inf());
constexpr float modff(float x) { float i = 0; return __builtin_modff(x, &i) + i * 10; }
constexpr long double modfl(long double x) { long double i = 0; return __builtin_modfl(x, &i) + i * 10; }
static_assert(modff(2.5f) == 20.5f && modfl(1.75L) == 10.75L);
