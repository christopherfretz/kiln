// [c.math.syn], [cpp.cond], [expr.const]: fmaximum_num / fminimum_num
// (C23 7.12.12.5-6) and sincos are builtins clang 22 added: declared only
// at -fclang-compat=22, with clang 22's types, and fmaximum_num /
// fminimum_num fold as clang 22 folds them (LLVM `maximumnum` /
// `minimumnum`: a NaN operand yields the other, two NaNs the second one
// quieted, `-0 < +0`), bit-exactly; sincos is not a constant expression.
// At 19: c-math-builtins-19.cpp. Codegen: builtins_c_math_22.cpp (WP-410).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 73:38 note "subexpression not valid in a constant expression"
//@ diag: 74:15 note "in call to 'sc()'"
static_assert(__has_builtin(__builtin_fmaximum_num) && __has_builtin(__builtin_fmaximum_numf) &&
              __has_builtin(__builtin_fmaximum_numl));
static_assert(__has_builtin(__builtin_fminimum_num) && __has_builtin(__builtin_fminimum_numf) &&
              __has_builtin(__builtin_fminimum_numl));
static_assert(__has_builtin(__builtin_sincos) && __has_builtin(__builtin_sincosf) &&
              __has_builtin(__builtin_sincosl));
static_assert(__has_constexpr_builtin(__builtin_fmaximum_num) &&
              __has_constexpr_builtin(__builtin_fminimum_numl));
static_assert(!__has_constexpr_builtin(__builtin_sincos));

extern "C" {
double __builtin_fmaximum_num(double, double) noexcept;
float __builtin_fmaximum_numf(float, float) noexcept;
long double __builtin_fminimum_numl(long double, long double) noexcept;
void __builtin_sincos(double, double *, double *) noexcept;
void __builtin_sincosf(float, float *, float *) noexcept;
void __builtin_sincosl(long double, long double *, long double *) noexcept;
}

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

void f(double d, float x, long double l, double *p) {
  static_assert(same<decltype(__builtin_fmaximum_numl(l, l)), long double>);
  static_assert(same<decltype(__builtin_fminimum_numf(x, x)), float>);
  static_assert(same<decltype(__builtin_sincos(d, p, p)), void>);
  static_assert(noexcept(__builtin_fminimum_num(d, d)) && noexcept(__builtin_sincos(d, p, p)));
}

using u64 = unsigned long long;
using u32 = unsigned;
#define D(v) __builtin_bit_cast(u64, (double)(v))
#define F(v) __builtin_bit_cast(u32, (float)(v))
constexpr double snan = __builtin_nans("5"); // 0x7ff0000000000005
constexpr double qnan = __builtin_nan("3");  // 0x7ff8000000000003
constexpr float snanf = __builtin_nansf("5"); // 0x7f800005
constexpr float qnanf = __builtin_nanf("3");  // 0x7fc00003

static_assert(D(__builtin_fmaximum_num(-0.0, 0.0)) == 0);
static_assert(D(__builtin_fmaximum_num(0.0, -0.0)) == 0);
static_assert(D(__builtin_fminimum_num(-0.0, 0.0)) == 0x8000000000000000);
static_assert(D(__builtin_fminimum_num(0.0, -0.0)) == 0x8000000000000000);
static_assert(__builtin_fmaximum_num(qnan, 1.0) == 1.0 && __builtin_fmaximum_num(1.0, qnan) == 1.0);
static_assert(__builtin_fmaximum_num(snan, 1.0) == 1.0 && __builtin_fminimum_num(1.0, snan) == 1.0);
static_assert(D(__builtin_fmaximum_num(snan, qnan)) == 0x7ff8000000000003);
static_assert(D(__builtin_fmaximum_num(qnan, snan)) == 0x7ff8000000000005);
static_assert(D(__builtin_fminimum_num(-snan, -snan)) == 0xfff8000000000005);
static_assert(__builtin_fminimum_num(1.5, -2.5) == -2.5 && __builtin_fmaximum_num(1.5, -2.5) == 1.5);

static_assert(F(__builtin_fminimum_numf(-0.0f, 0.0f)) == 0x80000000);
static_assert(F(__builtin_fmaximum_numf(snanf, qnanf)) == 0x7fc00003);
static_assert(F(__builtin_fminimum_numf(qnanf, snanf)) == 0x7fc00005);
static_assert(__builtin_fmaximum_numf(snanf, 2.0f) == 2.0f);

static_assert(__builtin_fmaximum_numl(-0.0L, 0.0L) == 0 && !__builtin_signbit(__builtin_fmaximum_numl(-0.0L, 0.0L)));
static_assert(__builtin_signbit(__builtin_fminimum_numl(0.0L, -0.0L)));
static_assert(__builtin_fminimum_numl(__builtin_nansl("1"), 3.0L) == 3.0L);
static_assert(__builtin_isnan(__builtin_fmaximum_numl(__builtin_nanl(""), __builtin_nansl(""))));

constexpr double sc() { double s, c; __builtin_sincos(0.0, &s, &c); return s + c; }
static_assert(sc() == 1.0);
