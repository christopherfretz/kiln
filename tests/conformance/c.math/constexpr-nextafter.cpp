// [c.math.syn] (constexpr nextafter, nexttoward; P0533), C23 7.12.11.3-4 and
// F.10.8.3: the next representable value of x's type in the direction of y
// (y itself when x == y; NaN in, NaN out), stepping through the subnormals
// and from zero to the least subnormal. nexttoward takes y as long double.
// A result that overflows, is subnormal or is zero raises an exception and
// is not a constant (constexpr-errors.cpp); x87 long double is checked in
// constexpr-float-kinds.cpp.
// Expectations from g++ 14, which folds the same calls.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 constant-evaluate neither nextafter nor nexttoward
constexpr bool neg(double x) { return __builtin_signbit(x); }

static_assert(__builtin_nextafter(1.0, 2.0) == 1 + 0x1p-52 && __builtin_nextafter(1.0, 0.0) == 1 - 0x1p-53);
static_assert(__builtin_nextafter(-1.0, 0.0) == -1 + 0x1p-53 && __builtin_nextafter(-1.0, -2.0) == -1 - 0x1p-52);
static_assert(__builtin_nextafter(0x1p-1022, 1.0) == 0x1.0000000000001p-1022);
static_assert(__builtin_nextafter(0x1.fffffffffffffp1023, 0.0) == 0x1.ffffffffffffep1023);
static_assert(__builtin_nextafter(__builtin_inf(), 0.0) == 0x1.fffffffffffffp1023);
static_assert(__builtin_nextafter(-__builtin_inf(), 0.0) == -0x1.fffffffffffffp1023);
static_assert(__builtin_nextafter(2.0, 2.0) == 2.0 && neg(__builtin_nextafter(0.0, -0.0)));
static_assert(__builtin_nextafter(1.0, __builtin_inf()) == 1 + 0x1p-52);
static_assert(__builtin_nextafter(__builtin_nan(""), 1.0) != __builtin_nextafter(__builtin_nan(""), 1.0));
static_assert(__builtin_nextafter(1.0, __builtin_nan("")) != 0);
static_assert(__builtin_nextafterf(1.0f, 2.0f) == 1 + 0x1p-23f && __builtin_nextafterf(0x1p-126f, 2.0f) == 0x1.000002p-126f);
static_assert(__builtin_nextafterf(-0x1p-126f, -1.0f) == -0x1.000002p-126f);
static_assert(__builtin_nextafterl(1.0L, 2.0L) == 1 + __LDBL_EPSILON__);
static_assert(__builtin_nextafterl(1.0L, 0.0L) == 1 - __LDBL_EPSILON__ / 2);

static_assert(__builtin_nexttoward(1.0, 2.0L) == 1 + 0x1p-52 && __builtin_nexttowardf(1.0f, 0.0L) == 1 - 0x1p-24f);
// y is compared in long double, not rounded to double first: 1 + LDBL_EPSILON
// is above 1.0 (even where it would round to 1.0 as a double), so x steps up.
static_assert(__builtin_nexttoward(1.0, 1 + __LDBL_EPSILON__) == 1 + 0x1p-52);
static_assert(__builtin_nexttowardf(1.0f, 1 + __LDBL_EPSILON__) == 1 + 0x1p-23f);
static_assert(__builtin_nexttowardf(1.0f, 1.0L) == 1.0f && __builtin_nexttowardl(1.0L, 2.0L) == 1 + __LDBL_EPSILON__);
