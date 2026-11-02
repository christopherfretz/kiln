// [basic.extended.fp], [c.math.syn] (constexpr nextafter, P0533), C23
// 7.12.11.3 and F.10.8.3: GCC's __builtin_nextafterf16 / f16b are constant
// expressions with exact IEEE results in binary16 / bfloat16: one step of
// the encoding away from or toward zero, equal operands give y (the sign
// of a zero y), infinity steps to the largest finite value, a NaN operand
// gives a NaN. Expectations from g++ 14 (which folds the same cases;
// subnormal, zero and overflowing results are rejected, see
// c.math/constexpr-errors.cpp).
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 do not declare __builtin_nextafterf16/f16b
#define NA __builtin_nextafterf16
#define NB __builtin_nextafterf16b
constexpr bool sb(_Float16 x) { return __builtin_signbit(x); }
constexpr bool sbb(__bf16 x) { return __builtin_signbit(x); }
static_assert(NA(1.0f16, 2.0f16) == 1.0f16 + 0x1p-10f16);
static_assert(NA(1.0f16, 0.0f16) == 1.0f16 - 0x1p-11f16);
static_assert(NA(-1.0f16, -2.0f16) == -1.0f16 - 0x1p-10f16);
static_assert(NA(-1.0f16, 1.0f16) == -1.0f16 + 0x1p-11f16);
static_assert(NA(2.0f16, 2.0f16) == 2.0f16);
static_assert(NA(-0.0f16, 0.0f16) == 0 && !sb(NA(-0.0f16, 0.0f16)));
static_assert(NA(0.0f16, -0.0f16) == 0 && sb(NA(0.0f16, -0.0f16)));
static_assert(NA(__builtin_inff16(), 0.0f16) == 0x1.ffcp15f16);
static_assert(NA(-__builtin_inff16(), 0.0f16) == -0x1.ffcp15f16);
static_assert(NA(0x1.ffcp15f16, 0.0f16) == 0x1.ff8p15f16);
static_assert(NA(0x1p-14f16, 1.0f16) == 0x1.004p-14f16);
static_assert(NA(__builtin_nanf16(""), 1.0f16) != NA(__builtin_nanf16(""), 1.0f16));
static_assert(NA(1.0f16, __builtin_nanf16("")) != 0);
static_assert(NB(1.0bf16, 2.0bf16) == 1.0bf16 + 0x1p-7bf16);
static_assert(NB(1.0bf16, 0.0bf16) == 1.0bf16 - 0x1p-8bf16);
static_assert(NB(-1.0bf16, -2.0bf16) == -1.0bf16 - 0x1p-7bf16);
static_assert(NB(static_cast<__bf16>(__builtin_inff()), 0.0bf16) == 0x1.fep127bf16);
static_assert(NB(0x1p-126bf16, 1.0bf16) == 0x1.02p-126bf16);
static_assert(NB(-0.0bf16, 0.0bf16) == 0 && !sbb(NB(-0.0bf16, 0.0bf16)));
