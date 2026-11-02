// [c.math.syn] (constexpr sqrt, P1383), [library.c]/3, IEEE 754-2019
// §5.4.1 squareRoot, C23 F.10.4.5: __builtin_sqrt is a constant expression
// in every format kiln has, correctly rounded to nearest-even: sqrt(-0) is
// -0, sqrt(+inf) is +inf and a quiet NaN is returned as is (no exception;
// g++ 14 folds neither, kiln follows IEEE and [library.c]/3 here). A
// negative operand or a signaling NaN raises invalid and is not a constant
// (constexpr-sqrt-errors.cpp). long double is checked in its target
// format, selected by __LDBL_MANT_DIG__ so the test is target-neutral;
// __builtin_sqrtf16 is clang's __fp16 form. Finite expectations from
// g++ 14 and the formats' definitions.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 constant-evaluate no sqrt builtin
constexpr bool neg(double x) { return __builtin_signbit(x); }

// binary64: exact squares, a correctly rounded case, the extremes.
static_assert(__builtin_sqrt(4.0) == 2.0 && __builtin_sqrt(1.0) == 1.0);
static_assert(__builtin_sqrt(2.0) == 1.4142135623730951);
static_assert(__builtin_sqrt(2.0) == 0x1.6a09e667f3bcdp0);
static_assert(__builtin_sqrt(0x1p-1074) == 0x1p-537 && __builtin_sqrt(0x1p-1073) == 0x1.6a09e667f3bcdp-537);
static_assert(__builtin_sqrt(0x1.fffffffffffffp1023) == 0x1.fffffffffffffp511);
static_assert(__builtin_sqrt(0.0) == 0 && !neg(__builtin_sqrt(0.0)));
static_assert(__builtin_sqrt(-0.0) == 0 && neg(__builtin_sqrt(-0.0)));
static_assert(__builtin_sqrt(__builtin_inf()) == __builtin_inf());
static_assert(__builtin_sqrt(__builtin_nan("")) != __builtin_sqrt(__builtin_nan("")));

// binary32.
static_assert(__builtin_sqrtf(16.0f) == 4.0f && __builtin_sqrtf(2.0f) == 0x1.6a09e6p0f);
static_assert(__builtin_sqrtf(0x1p-149f) == 0x1.6a09e6p-75f && __builtin_sqrtf(-0.0f) == 0);

// binary128.
static_assert(__builtin_sqrtf128(9.0f128) == 3);
static_assert(__builtin_sqrtf128(2.0f128) == 0x1.6a09e667f3bcc908b2fb1366ea95p0f128);
static_assert(__builtin_sqrtf128(0x1p-16494f128) == 0x1p-8247f128);

// long double, in the target's format.
static_assert(__builtin_sqrtl(0.25L) == 0.5L);
#if __LDBL_MANT_DIG__ == 64
static_assert(__builtin_sqrtl(2.0L) == 0x1.6a09e667f3bcc908p0L);
static_assert(__builtin_sqrtl(0x1p-16445L) == 0x1.6a09e667f3bcc908p-8223L);
#elif __LDBL_MANT_DIG__ == 113
static_assert(__builtin_sqrtl(2.0L) == 0x1.6a09e667f3bcc908b2fb1366ea95p0L);
#else
static_assert(__builtin_sqrtl(2.0L) == 0x1.6a09e667f3bcdp0L);
#endif

// binary16 (clang's __fp16 form, rounded once to 11 bits), and _Float16 /
// _Float32 / _Float64 through the builtins they convert to.
static_assert(__builtin_sqrtf16(4) == 2 && (float)__builtin_sqrtf16(2) == 0x1.6ap0f);
static_assert(_Float16(__builtin_sqrtf(2.0f16)) == 0x1.6ap0f16);
static_assert(__builtin_sqrtf(2.0f32) == 0x1.6a09e6p0f32 && __builtin_sqrt(2.0f64) == 0x1.6a09e667f3bcdp0f64);

// In a constexpr function and as a template argument.
constexpr double hyp(double a, double b) { return __builtin_sqrt(a * a + b * b); }
static_assert(hyp(3, 4) == 5);
template <int N> struct I { static constexpr int v = N; };
static_assert(I<int(__builtin_sqrt(1764.0))>::v == 42);
