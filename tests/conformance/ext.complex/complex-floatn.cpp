// [basic.extended.fp], [conv.rank], GNU _Complex extension: `_Complex` of
// the P1467 types _Float32, _Float64 and _Float128 (GCC 14 accepts them;
// clang 19 has no _FloatN in C++). Each is a pair of its element (size
// and alignment twice / once the element's), converts by the extended
// types' ranks (equal rank: the greater subrank, so _Float32 wins over
// float and _Float64 over double; a lower-rank operand widens), and its
// arithmetic constant-evaluates in the element's format (Annex G
// multiplication/division as for _Complex double).
// (Compound assignment of a _Complex is not constant-evaluated, as in
// clang 19; GCC evaluates it.)
// Expectations from g++ 14, which accepts this file.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19 has no _Float32/_Float64/_Float128 in C++
template<class T, class U> constexpr bool same = false;
template<class T> constexpr bool same<T, T> = true;
using C32 = _Complex _Float32;
using C64 = _Complex _Float64;
using C128 = _Complex _Float128;

static_assert(sizeof(C32) == 8 && alignof(C32) == 4);
static_assert(sizeof(C64) == 16 && alignof(C64) == 8);
static_assert(sizeof(C128) == 32 && alignof(C128) == 16);
static_assert(!same<C32, _Complex float> && !same<C64, _Complex double>);
static_assert(same<decltype(__real__ C32()), _Float32> && same<decltype(__imag__ C128()), _Float128>);

constexpr C32 a = {1.0f32, 2.0f32};
constexpr C64 b = {3.0f64, 4.0f64};
constexpr C128 c = {1.0f128, -1.0f128};
static_assert(same<decltype(a + 1.0f32), C32> && same<decltype(a + 1.0f), C32>);
static_assert(same<decltype(a + b), C64> && same<decltype(b * 2.0), C64>);
static_assert(same<decltype(a + c), C128> && same<decltype(b / c), C128>);
static_assert(same<decltype(a + (_Complex float)0), C32> && same<decltype(b - (_Complex double)0), C64>);

static_assert(__real__ (a * a) == -3.0f32 && __imag__ (a * a) == 4.0f32);
static_assert(__real__ (a + a) == 2.0f32 && __imag__ (a - a) == 0);
static_assert(__real__ (b / b) == 1 && __imag__ (b / b) == 0);
static_assert(__real__ (b * b) == -7 && __imag__ (b * b) == 24);
static_assert(__real__ (c * c) == 0 && __imag__ (c * c) == -2);
static_assert(__real__ (c / c) == 1 && __imag__ (c / c) == 0);
// binary128 keeps what binary64 rounds away.
static_assert(__real__ (C128{1 + 0x1p-100f128, 0} * C128{1, 0}) == 1 + 0x1p-100f128);
static_assert(__imag__ (~a) == -2.0f32 && __real__ (-c) == -1);
constexpr C64 acc() { C64 z = {1, 1}; for (int i = 0; i < 4; ++i) z = z * z; return z; }
static_assert(__real__ acc() == 256 && __imag__ acc() == 0);
static_assert(a == C32{1, 2} && c != C128{1, 1});
