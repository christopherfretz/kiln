// [expr.const], C23 7.6.2 (`#pragma STDC FENV_ROUND`): constant evaluation
// rounds each operation in the mode declared where the operation is
// written, as clang does: `+ - * /`, compound assignment, `++`/`--`,
// floating and integer-to-floating conversions and floating literals, for
// float, double, long double, _Float16 and __bf16. Overflow goes to the
// largest finite value in the directions that round toward zero, an exact
// zero difference is -0 only downward, a tie rounds away from zero under
// FE_TONEARESTFROMZERO, and FE_DYNAMIC (outside FENV_ACCESS ON) and
// FE_TONEAREST round to nearest. A function or template keeps the mode of
// its definition; a block-scope pragma lasts to the end of its block.
// clang 22 gives the same values (WP-1744).
//@ expect: compile-ok
//@ requires: clang>=22
//@ strict-diags
//@ args: -std=c++26 -Wno-unknown-pragmas
using u16 = unsigned short;
using u32 = unsigned int;
using u64 = unsigned long long;
constexpr u64 bits(double d) { return __builtin_bit_cast(u64, d); }
constexpr u32 bits(float f) { return __builtin_bit_cast(u32, f); }
constexpr u16 bits(_Float16 f) { return __builtin_bit_cast(u16, f); }
constexpr u16 bits(__bf16 f) { return __builtin_bit_cast(u16, f); }
constexpr double dmax = __DBL_MAX__;
constexpr double dmin = __DBL_DENORM_MIN__;
constexpr bool neg_zero(double d) { return d == 0 && __builtin_signbit(d); }

#pragma STDC FENV_ROUND FE_UPWARD
constexpr double up_third = 1.0 / 3.0;
constexpr double up_neg_third = -1.0 / 3.0;
constexpr double up_over = dmax * 2;
constexpr double up_neg_over = -dmax * 2;
constexpr double up_tiny = dmin / 2;
constexpr double up_zero = 1.0 - 1.0;
constexpr float up_narrow = 1.0 / 3.0;
constexpr float up_int = 16777217;
constexpr double up_lit = 0.1;
constexpr float up_litf = 0.1f;
constexpr double up_inc = [] { double x = 0x1p53; ++x; return x; }();
constexpr float up_div = [] { float x = 1; x /= 3; return x; }();
constexpr _Float16 up_h = (_Float16)1 / (_Float16)3;
constexpr __bf16 up_b = (__bf16)1 / (__bf16)3;
constexpr long double up_l = 1.0L / 3.0L;
constexpr double up_fn(double a, double b) { return a / b; }
template <class T> constexpr T up_tmpl(T a, T b) { return a / b; }

#pragma STDC FENV_ROUND FE_DOWNWARD
constexpr double down_third = 1.0 / 3.0;
constexpr double down_neg_third = -1.0 / 3.0;
constexpr double down_over = dmax * 2;
constexpr double down_zero = 1.0 - 1.0;
constexpr float down_narrow = 1.0 / 3.0;
constexpr float down_int = 16777217;
constexpr double down_lit = 0.1;
constexpr float down_litf = 0.1f;
constexpr double down_dec = [] { double x = -0x1p53; --x; return x; }();
constexpr _Float16 down_h = (_Float16)1 / (_Float16)3;
constexpr __bf16 down_b = (__bf16)1 / (__bf16)3;
constexpr long double down_l = 1.0L / 3.0L;
// Defined upward, used here: the definition's mode.
constexpr double down_uses_up_fn = up_fn(1.0, 3.0);
constexpr double down_uses_up_tmpl = up_tmpl(1.0, 3.0);

#pragma STDC FENV_ROUND FE_TOWARDZERO
constexpr double zero_third = 1.0 / 3.0;
constexpr double zero_neg_third = -1.0 / 3.0;
constexpr double zero_over = -dmax * 2;
constexpr double zero_scoped = [] {
  double a = 1.0 / 3.0;
  {
#pragma STDC FENV_ROUND FE_UPWARD
    a += 1.0 / 3.0;
  }
  return a - 1.0 / 3.0;
}();

#pragma STDC FENV_ROUND FE_TONEARESTFROMZERO
constexpr double away_tie = 0x1p53 + 1.0;
constexpr double away_tiny = dmin / 2;
constexpr double away_third = 1.0 / 3.0;

#pragma STDC FENV_ROUND FE_DYNAMIC
constexpr double dyn_tie = 0x1p53 + 1.0;
constexpr double dyn_tiny = dmin / 2;

#pragma STDC FENV_ROUND FE_TONEAREST
constexpr double near_third = 1.0 / 3.0;
constexpr double near_neg_third = -1.0 / 3.0;

static_assert(bits(near_third) == 0x3FD5555555555555);
static_assert(bits(up_third) == 0x3FD5555555555556);
static_assert(bits(down_third) == 0x3FD5555555555555);
static_assert(bits(zero_third) == 0x3FD5555555555555);
static_assert(bits(away_third) == 0x3FD5555555555555);
static_assert(bits(up_neg_third) == bits(near_neg_third));
static_assert(bits(down_neg_third) == 0xBFD5555555555556);
static_assert(bits(zero_neg_third) == bits(near_neg_third));

static_assert(up_over == __builtin_huge_val());
static_assert(up_neg_over == -dmax);
static_assert(down_over == dmax);
static_assert(zero_over == -dmax);
static_assert(up_tiny == dmin && away_tiny == dmin && dyn_tiny == 0);
static_assert(neg_zero(down_zero) && up_zero == 0 && !__builtin_signbit(up_zero));

static_assert(bits(up_narrow) == 0x3EAAAAAB && bits(down_narrow) == 0x3EAAAAAA);
static_assert(up_int == 16777218.0f && down_int == 16777216.0f);
static_assert(bits(up_lit) == 0x3FB999999999999A && bits(down_lit) == 0x3FB9999999999999);
static_assert(bits(up_litf) == 0x3DCCCCCD && bits(down_litf) == 0x3DCCCCCC);
static_assert(up_inc == 0x1p53 + 2 && down_dec == -0x1p53 - 2);
static_assert(bits(up_div) == 0x3EAAAAAB);
static_assert(bits(up_h) == bits(down_h) + 1 && bits(up_b) == bits(down_b) + 1);
static_assert(up_l > down_l);

static_assert(bits(down_uses_up_fn) == bits(up_third));
static_assert(bits(down_uses_up_tmpl) == bits(up_third));
static_assert(bits(zero_scoped) == 0x3FD5555555555557);
static_assert(away_tie == 0x1p53 + 2 && dyn_tie == 0x1p53);
