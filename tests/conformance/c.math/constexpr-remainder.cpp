// [c.math.syn] (constexpr fmod, remainder, remquo; P0533), C23 7.12.10 and
// F.10.7: the remainders are exact in every format. fmod truncates the
// quotient and keeps x's sign (a zero result too); remainder rounds the
// quotient to nearest even; remquo also stores at least the three low
// bits of that quotient with its sign. An infinite y returns x; a zero y
// or an infinite x is invalid (see constexpr-errors.cpp). Expectations
// from g++ 14, which folds the same calls (but not fmod(x, inf), whose
// exact result x kiln gives).
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 constant-evaluate no remainder builtin
constexpr bool neg(double x) { return __builtin_signbit(x); }

static_assert(__builtin_fmod(7.0, 3.0) == 1.0 && __builtin_fmod(-7.0, 3.0) == -1.0);
static_assert(__builtin_fmod(7.0, -3.0) == 1.0 && __builtin_fmodf(5.5f, 2.0f) == 1.5f);
static_assert(__builtin_fmod(-6.0, 3.0) == 0 && neg(__builtin_fmod(-6.0, 3.0)));
static_assert(__builtin_fmod(0x1p1023, 3.0) == 2.0 && __builtin_fmod(1e308, 0x1p-1074) == 0);
static_assert(__builtin_fmod(0x1.8p-1073, 0x1p-1073) == 0x1p-1074 && __builtin_fmod(0.1, 0x1p-1060) == 0);
static_assert(__builtin_fmod(2.0, __builtin_inf()) == 2.0 && __builtin_fmodl(10.5L, 4.0L) == 2.5L);
static_assert(__builtin_remainder(7.0, 2.0) == -1.0 && __builtin_remainder(5.0, 2.0) == 1.0);
static_assert(__builtin_remainder(-7.0, 2.0) == 1.0 && __builtin_remainderf(5.25f, 1.0f) == 0.25f);
static_assert(__builtin_remainder(1.0, 0x1p-1074) == 0 && __builtin_remainderl(10.0L, 4.0L) == 2.0L);
static_assert(__builtin_remainder(-4.0, 2.0) == 0 && neg(__builtin_remainder(-4.0, 2.0)));

constexpr int quo(double x, double y) { int q = 99; __builtin_remquo(x, y, &q); return q & ~0x7fffff00 | (q < 0 ? 0x100 : 0); }
constexpr double rem(double x, double y) { int q = 0; return __builtin_remquo(x, y, &q); }
static_assert(rem(7.0, 2.0) == -1.0 && (quo(7.0, 2.0) & 7) == 4);
static_assert(rem(-7.0, 2.0) == 1.0 && (quo(-7.0, 2.0) & 0x107) == 0x104);
static_assert(rem(29.0, 3.0) == -1.0 && (quo(29.0, 3.0) & 7) == 2);
constexpr int quof() { int q = 0; float r = __builtin_remquof(-9.0f, -4.0f, &q); return q * 10 + int(r); }
static_assert(quof() == 19);
constexpr int quol() { int q = 0; long double r = __builtin_remquol(10.0L, 3.0L, &q); return q * 10 + int(r); }
static_assert(quol() == 31);
