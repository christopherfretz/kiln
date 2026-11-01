// [expr.const], [c.math.fpclass]: __builtin_isfpclass(x, mask) is
// constant-evaluable: whether x's class is in the LLVM FPClassTest mask
// (0x1 sNaN, 0x2 qNaN, 0x4 -inf, 0x8 -normal, 0x10 -subnormal, 0x20 -zero,
// 0x40 +zero, 0x80 +subnormal, 0x100 +normal, 0x200 +inf).
// __builtin_expect_with_probability evaluates to its first operand.
//@ expect: compile-ok
//@ strict-diags
static_assert(__builtin_isfpclass(1.0, 0x100));
static_assert(!__builtin_isfpclass(-1.0, 0x100));
static_assert(__builtin_isfpclass(-1.0f, 0x008));
static_assert(__builtin_isfpclass(__builtin_nan(""), 0x002));
static_assert(!__builtin_isfpclass(__builtin_nan(""), 0x001));
static_assert(__builtin_isfpclass(__builtin_nans(""), 0x001));
static_assert(__builtin_isfpclass(-0.0, 0x020));
static_assert(__builtin_isfpclass(0.0L, 0x040));
static_assert(__builtin_isfpclass(-__builtin_inf(), 0x004));
static_assert(__builtin_isfpclass(1e-310, 0x080));
static_assert(__builtin_isfpclass(-1e-310, 0x3ff));
static_assert(!__builtin_isfpclass(1.0, 0));
static_assert(__builtin_expect_with_probability(5, 1, 0.5) == 5);
constexpr long e = __builtin_expect_with_probability(7L, 7L, 1.0);
static_assert(e == 7);
template <class T> constexpr bool pos_normal(T x) { return __builtin_isfpclass(x, 0x100); }
static_assert(pos_normal(2.0f) && !pos_normal(-2.0) && !pos_normal(0.0L));
template <int M> constexpr bool cls(double x) { return __builtin_isfpclass(x, M); }
static_assert(cls<0x200>(__builtin_inf()) && !cls<0x004>(__builtin_inf()));
