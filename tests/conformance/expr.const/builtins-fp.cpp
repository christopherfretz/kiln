// [expr.const], [c.math.fpclass], [numeric.limits.members]: the float
// constants (huge_val, nan, nans) and the classification builtins that
// libc++'s <cmath> and numeric_limits use are constant-evaluable.
//@ expect: compile-ok
//@ strict-diags
constexpr double inf = __builtin_huge_val();
constexpr float inff = __builtin_huge_valf();
constexpr long double infl = __builtin_huge_vall();
constexpr double qnan = __builtin_nan("");
constexpr float qnanf = __builtin_nanf("");
constexpr long double qnanl = __builtin_nanl("");
constexpr double snan = __builtin_nans("");
constexpr float snanf = __builtin_nansf("");
constexpr long double snanl = __builtin_nansl("");

static_assert(inf > 1e308 && inff > 3e38f && infl == inf);
static_assert(__builtin_isinf(inf));
static_assert(__builtin_isinf(-inff));
static_assert(__builtin_isinf(infl));
static_assert(!__builtin_isinf(1.0));
static_assert(!__builtin_isinf(qnan));
static_assert(__builtin_isnan(qnan));
static_assert(__builtin_isnan(qnanf));
static_assert(__builtin_isnan(qnanl));
static_assert(__builtin_isnan(snan));
static_assert(__builtin_isnan(snanf));
static_assert(__builtin_isnan(snanl));
static_assert(!__builtin_isnan(inf));
static_assert(!__builtin_isnan(0.0f));
static_assert(qnan != qnan);
static_assert(__builtin_isfinite(1.0));
static_assert(__builtin_isfinite(-0.0f));
static_assert(__builtin_isfinite(1e-310));
static_assert(!__builtin_isfinite(inf));
static_assert(!__builtin_isfinite(qnanf));
static_assert(__builtin_isnormal(1.0));
static_assert(__builtin_isnormal(-2.5f));
static_assert(__builtin_isnormal(1.0L));
static_assert(!__builtin_isnormal(0.0));
static_assert(!__builtin_isnormal(1e-310));
static_assert(!__builtin_isnormal(inf));
static_assert(!__builtin_isnormal(qnan));

// The payload string is parsed as an integer (decimal, octal or hex).
static_assert(__builtin_bit_cast(unsigned long long, __builtin_nan("")) == 0x7ff8000000000000ull);
static_assert(__builtin_bit_cast(unsigned long long, __builtin_nan("0x15")) == 0x7ff8000000000015ull);
static_assert(__builtin_bit_cast(unsigned, __builtin_nanf("10")) == 0x7fc0000au);
static_assert(__builtin_bit_cast(unsigned, __builtin_nanf("010")) == 0x7fc00008u);
static_assert(__builtin_bit_cast(unsigned long long, __builtin_nans("")) == 0x7ff4000000000000ull);
static_assert(__builtin_bit_cast(unsigned long long, __builtin_nans("1")) == 0x7ff0000000000001ull);
static_assert(__builtin_bit_cast(unsigned, __builtin_huge_valf()) == 0x7f800000u);
static_assert(__builtin_bit_cast(unsigned long long, __builtin_huge_val()) == 0x7ff0000000000000ull);

// numeric_limits-shaped uses.
struct limits_double {
  static constexpr double infinity() noexcept { return __builtin_huge_val(); }
  static constexpr double quiet_NaN() noexcept { return __builtin_nan(""); }
  static constexpr double signaling_NaN() noexcept { return __builtin_nans(""); }
};
static_assert(__builtin_isinf(limits_double::infinity()));
static_assert(__builtin_isnan(limits_double::quiet_NaN()));
static_assert(__builtin_isnan(limits_double::signaling_NaN()));

constexpr bool classify(double d) { return __builtin_isfinite(d) && !__builtin_isnan(d); }
static_assert(classify(3.0) && !classify(inf) && !classify(qnan));

