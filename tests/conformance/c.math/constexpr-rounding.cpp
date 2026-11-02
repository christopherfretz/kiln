// [c.math.syn] (constexpr ceil, floor, trunc, round, nearbyint, rint,
// lround, llround, lrint, llrint; P0533), C23 7.12.9 and F.10.6: the
// rounding builtins are constant expressions in every format with exact
// results: a zero result keeps the operand's sign, an integral, infinite
// or quiet NaN operand is returned as is, round breaks ties away from zero
// and rint/nearbyint/lrint round to nearest even (the default rounding
// mode of [library.c]/3; g++ 14 folds no rint form at all, kiln does). The
// integer forms are not constants when the value does not fit (see
// constexpr-errors.cpp).
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 constant-evaluate no rounding builtin
constexpr bool neg(double x) { return __builtin_signbit(x); }
constexpr bool nan(double x) { return x != x; }

static_assert(__builtin_ceil(1.5) == 2.0 && __builtin_ceil(-1.5) == -1.0);
static_assert(__builtin_ceil(-0.5) == 0 && neg(__builtin_ceil(-0.5)));
static_assert(__builtin_ceil(0x1p-1074) == 1.0 && __builtin_ceil(0x1.fffffffffffffp51) == 0x1p52);
static_assert(__builtin_floor(-1.5) == -2.0 && __builtin_floor(1.5f) == 1.0f);
static_assert(__builtin_floor(0.5) == 0 && !neg(__builtin_floor(0.5)));
static_assert(__builtin_floor(-0x1p-1074) == -1.0 && __builtin_floorl(-2.5L) == -3.0L);
static_assert(__builtin_trunc(-1.75) == -1.0 && __builtin_truncf(2.75f) == 2.0f);
static_assert(__builtin_trunc(-0.75) == 0 && neg(__builtin_trunc(-0.75)));
static_assert(__builtin_round(2.5) == 3.0 && __builtin_round(-2.5) == -3.0);
static_assert(__builtin_round(0.49999999999999994) == 0.0 && __builtin_roundf(-0.5f) == -1.0f);
static_assert(__builtin_roundl(1.5L) == 2.0L && __builtin_roundl(-0.25L) == 0);
static_assert(__builtin_rint(2.5) == 2.0 && __builtin_rint(3.5) == 4.0);
static_assert(__builtin_rintf(-2.5f) == -2.0f && neg(__builtin_rint(-0.5)));
static_assert(__builtin_nearbyint(0.5) == 0.0 && __builtin_nearbyintl(1.5L) == 2.0L);

// Integral, infinite and NaN operands.
static_assert(__builtin_ceil(0x1p60) == 0x1p60 && __builtin_floor(-__builtin_inf()) == -__builtin_inf());
static_assert(nan(__builtin_trunc(__builtin_nan(""))) && neg(__builtin_round(-0.0)));
static_assert(__builtin_ceilf(0x1p23f + 1) == 0x1p23f + 1);
static_assert(__builtin_floorl(-0x1p63L) == -0x1p63L);

// To integers: long and long long are 64 bits on both targets.
static_assert(__builtin_lround(2.5) == 3 && __builtin_lround(-2.5) == -3);
static_assert(__builtin_llround(-0.5f) == -1 && __builtin_lroundl(0.25L) == 0);
static_assert(__builtin_lrint(2.5) == 2 && __builtin_llrint(-3.5) == -4);
static_assert(__builtin_lrintf(0.5f) == 0 && __builtin_llrintl(-2.5L) == -2);
static_assert(__builtin_llround(-0x1p63) == -0x7fffffffffffffffLL - 1);
static_assert(__builtin_lround(0x1.fffffffffffffp62) == 0x7ffffffffffffc00L);
