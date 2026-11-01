// [expr.const], [c.math.abs], [c.math.fpclass], [c.math.syn]: the libm
// builtins clang 19 evaluates in constant expressions (fabs, copysign,
// fmax/fmin, inf, isinf_sign, fpclassify, issubnormal/iszero/issignaling,
// expect, assume), including signed zeros, NaN payloads and long double.
//@ expect: compile-ok
//@ strict-diags
using u64 = unsigned long long;
constexpr double inf = __builtin_inf();
constexpr float inff = __builtin_inff();
constexpr long double infl = __builtin_infl();
static_assert(inf == __builtin_huge_val() && inff == __builtin_huge_valf());
static_assert(infl == __builtin_huge_vall() && infl > 1e308L);

// fabs / copysign act on the sign bit, NaN payloads included.
static_assert(__builtin_fabs(-2.5) == 2.5 && __builtin_fabsf(-0.5f) == 0.5f);
static_assert(__builtin_fabsl(-3.0L) == 3.0L && __builtin_fabs(inf * -1) == inf);
static_assert(__builtin_bit_cast(u64, __builtin_fabs(-0.0)) == 0);
static_assert(__builtin_bit_cast(u64, __builtin_fabs(-__builtin_nan("0x123"))) ==
              0x7ff8000000000123ull);
static_assert(__builtin_bit_cast(u64, __builtin_copysign(__builtin_nan("7"), -1.0)) ==
              0xfff8000000000007ull);
static_assert(__builtin_bit_cast(u64, __builtin_copysign(1.0, -0.0)) == 0xbff0000000000000ull);
static_assert(__builtin_copysignf(3.0f, -1.0f) == -3.0f && __builtin_copysignl(-2.0L, 1.0L) == 2.0L);
static_assert(__builtin_bit_cast(unsigned, __builtin_copysignf(0.0f, -inff)) == 0x80000000u);
static_assert(__builtin_isnan(__builtin_fabsl(-__builtin_nanl(""))));

// fmax / fmin: a NaN operand yields the other; -0 < +0.
static_assert(__builtin_fmax(1.0, 3.0) == 3.0 && __builtin_fmin(1.0, 3.0) == 1.0);
static_assert(__builtin_fmaxf(-1.0f, -3.0f) == -1.0f && __builtin_fminl(2.0L, -2.0L) == -2.0L);
static_assert(__builtin_fmax(__builtin_nan(""), 2.0) == 2.0);
static_assert(__builtin_fmax(2.0, __builtin_nan("")) == 2.0);
static_assert(__builtin_fmin(__builtin_nanf(""), 2.0f) == 2.0f);
static_assert(__builtin_isnan(__builtin_fmin(__builtin_nan(""), __builtin_nan(""))));
static_assert(__builtin_bit_cast(u64, __builtin_fmax(-0.0, 0.0)) == 0);
static_assert(__builtin_bit_cast(u64, __builtin_fmax(0.0, -0.0)) == 0);
static_assert(__builtin_bit_cast(u64, __builtin_fmin(0.0, -0.0)) == 0x8000000000000000ull);
static_assert(__builtin_bit_cast(u64, __builtin_fmin(-0.0, 0.0)) == 0x8000000000000000ull);
static_assert(__builtin_fmaxl(inf, 1.0L) == infl && __builtin_fminl(-infl, 0.0L) == -infl);

// Classification.
static_assert(__builtin_isinf_sign(inf) == 1 && __builtin_isinf_sign(-inff) == -1);
static_assert(__builtin_isinf_sign(-infl) == -1 && __builtin_isinf_sign(1.0) == 0);
static_assert(__builtin_isinf_sign(__builtin_nan("")) == 0);
static_assert(__builtin_issubnormal(1e-310) && !__builtin_issubnormal(1.0));
static_assert(__builtin_issubnormal(1e-40f) && !__builtin_issubnormal(0.0f));
static_assert(__builtin_issubnormal(1e-4940L) && !__builtin_issubnormal(1e-4900L));
static_assert(__builtin_iszero(-0.0) == 1 && __builtin_iszero(0.0L) == 1);
static_assert(__builtin_iszero(1e-310) == 0);
static_assert(__builtin_issignaling(__builtin_nans("")) && !__builtin_issignaling(__builtin_nan("")));
static_assert(__builtin_issignaling(__builtin_nansl("1")) && !__builtin_issignaling(inf));
static_assert(__builtin_issignaling(__builtin_nansf("")) && !__builtin_issignaling(1.0f));

enum { NAN_ = 10, INF_, NORMAL_, SUB_, ZERO_ };
constexpr int cls(double x) { return __builtin_fpclassify(NAN_, INF_, NORMAL_, SUB_, ZERO_, x); }
static_assert(cls(__builtin_nan("")) == NAN_ && cls(-inf) == INF_ && cls(1.5) == NORMAL_);
static_assert(cls(1e-310) == SUB_ && cls(-0.0) == ZERO_);
static_assert(__builtin_fpclassify(0, 1, 2, 3, 4, 1e-40f) == 3);
static_assert(__builtin_fpclassify(0, 1, 2, 3, 4, 1e-4940L) == 3);
static_assert(__builtin_fpclassify(0, 1, 2, 3, 4, infl) == 1);
// Only the selected argument is evaluated.
constexpr int bad() { throw 0; }
static_assert(__builtin_fpclassify(bad(), bad(), 7, bad(), bad(), 2.0) == 7);
static_assert(__builtin_fpclassify(bad(), bad(), bad(), bad(), 'z', 0.0f) == 'z');

// expect is its first operand (the second is not evaluated); assume is
// not evaluated (a false assumption is not diagnosed).
static_assert(__builtin_expect(42, 0) == 42);
static_assert(__builtin_expect(3, bad()) == 3);
constexpr int f(int n) {
  __builtin_assume(n > 0);
  __builtin_assume(n > 100);
  return n;
}
static_assert(f(5) == 5);
