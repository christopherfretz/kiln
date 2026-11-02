// [c.math.syn] (constexpr fma, fdim; P0533), C23 7.12.12.1, 7.12.13.1, F.10.9:
// fma rounds x * y + z once (so it sees the low half of the product that
// x * y + z loses), with the sign of an exact zero sum as IEEE addition;
// fdim is x - y when positive, else +0. Infinity times zero, inf - inf and
// an overflowing result are not constants (constexpr-errors.cpp).
// Expectations from g++ 14, which folds the same calls except those with an
// infinite operand (its MPFR folder wants finite inputs); those are exact
// under F.10.9 and kiln evaluates them.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 constant-evaluate neither fma nor fdim
constexpr bool neg(double x) { return __builtin_signbit(x); }

static_assert(__builtin_fma(2.0, 3.0, 1.0) == 7.0 && __builtin_fmaf(0.1f, 10.0f, -1.0f) == 0x1p-26f);
// (1 + 2^-52)^2 - (1 + 2^-51) = 2^-104, lost by the unfused form.
static_assert(__builtin_fma(1 + 0x1p-52, 1 + 0x1p-52, -(1 + 0x1p-51)) == 0x1p-104);
static_assert((1 + 0x1p-52) * (1 + 0x1p-52) - (1 + 0x1p-51) == 0);
static_assert(__builtin_fma(0x1p-600, 0x1p-500, 1.0) == 1.0);
static_assert(__builtin_fma(1.0, -1.0, 1.0) == 0 && !neg(__builtin_fma(1.0, -1.0, 1.0)));
static_assert(neg(__builtin_fma(-0.0, 1.0, -0.0)) && !neg(__builtin_fma(-0.0, 1.0, 0.0)));
static_assert(__builtin_fma(__builtin_inf(), 2.0, 1.0) == __builtin_inf());
static_assert(__builtin_fmal(3.0L, 0.5L, 0.25L) == 1.75L);
static_assert(__builtin_fma(0x1p1023, 2.0, -0x1p1023) == 0x1p1023);
static_assert(__builtin_fdim(3.0, 1.0) == 2.0 && __builtin_fdim(1.0, 3.0) == 0);
static_assert(!neg(__builtin_fdim(-1.0, -0.5)) && __builtin_fdimf(2.5f, -0.5f) == 3.0f);
static_assert(__builtin_fdiml(1.0L, 0.25L) == 0.75L && __builtin_fdim(__builtin_inf(), 1.0) == __builtin_inf());
