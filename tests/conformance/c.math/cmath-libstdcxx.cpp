// [c.math.syn] (P0533 through libstdc++'s <cmath>): libstdc++ 14 implements
// the float, long double and std::float*_t overloads of the P0533 set as
// constexpr functions over the __builtin_* math functions, so they are
// constant through the std:: names (rounding, remainder, fdim, fma, the
// exponent functions, nextafter/nexttoward, signbit and the comparison
// macros; nextafter(_Float16/__bf16) via __builtin_nextafterf16/f16b).
// Its double overloads are `using ::ceil;` etc., C library declarations
// GCC folds as implicit builtins; kiln does not yet treat them as builtins,
// so this test calls only the overloads libstdc++ defines itself.
// frexp/modf/remquo are not constexpr in libstdc++ 14.
// Expectations from g++ 14, which accepts this file.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ skip(clang): clang 19 constant-evaluates none of these builtins
#include <cmath>
#include <cstdlib>
#include <stdfloat>

static_assert(std::ceil(-1.5f) == -1.0f && std::ceil(1.5L) == 2.0L);
static_assert(std::floor(-1.5L) == -2.0L && std::trunc(-1.5f) == -1.0f && std::round(2.5L) == 3.0L);
static_assert(std::lround(-2.5L) == -3 && std::llround(2.5f) == 3);
static_assert(std::fmod(7.0L, 3.0L) == 1.0L && std::remainder(7.0f, 2.0f) == -1.0f);
static_assert(std::fdim(3.0f, 1.0f) == 2.0f && std::fdim(1.0L, 3.0L) == 0.0L);
static_assert(std::fma(2.0L, 3.0L, 1.0L) == 7.0L && std::fma(0.1f, 10.0f, -1.0f) == 0x1p-26f);
static_assert(std::ldexp(1.0L, 4) == 16.0L && std::scalbn(1.0f, -1) == 0.5f);
static_assert(std::scalbln(3.0L, 2L) == 12.0L);
static_assert(std::ilogb(1024.0f) == 10 && std::logb(0.25L) == -2.0L);
static_assert(std::nextafter(1.0f, 2.0f) == 1.0f + 0x1p-23f);
static_assert(std::nexttoward(1.0f, 0.0L) == 1.0f - 0x1p-24f);
static_assert(std::signbit(-0.0) && !std::signbit(0.0f) && std::signbit(-1.0L));
static_assert(std::isgreater(2.0, 1.0) && std::isgreaterequal(1.0f, 1.0f));
static_assert(std::isless(1.0L, 2.0L) && std::islessequal(1.0, 1.0));
static_assert(std::islessgreater(1.0, 2.0) && !std::islessgreater(1.0, 1.0));
static_assert(std::isunordered(__builtin_nan(""), 1.0) && !std::isless(__builtin_nanf(""), 1.0f));
static_assert(std::fpclassify(0x1p-1040) == FP_SUBNORMAL && std::isnormal(1.0f));
static_assert(std::copysign(1.0f, -0.0f) == -1.0f && std::fmax(1.0f, 2.0f) == 2.0f);

static_assert(std::abs(-1.5) == 1.5 && std::fabs(-1.5f) == 1.5f);

#if defined(__STDCPP_FLOAT16_T__)
static_assert(std::ceil(1.5f16) == 2.0f16 && std::fma(2.0f16, 3.0f16, 1.0f16) == 7.0f16);
static_assert(std::nextafter(1.0f16, 2.0f16) == 1.0f16 + 0x1p-10f16);
static_assert(std::signbit(-1.0f16) && std::isless(1.0f16, 2.0f16));
#endif
#if defined(__STDCPP_BFLOAT16_T__)
static_assert(std::floor(-1.5bf16) == -2.0bf16);
static_assert(std::nextafter(1.0bf16, 2.0bf16) == 1.0bf16 + 0x1p-7bf16);
#endif
#if defined(__STDCPP_FLOAT64_T__)
static_assert(std::trunc(-2.5f64) == -2.0f64 && std::ilogb(0x1p-1022f64) == -1022);
#endif
#if defined(__STDCPP_FLOAT128_T__)
static_assert(std::round(2.5f128) == 3.0f128 && std::fmod(7.0f128, 3.0f128) == 1.0f128);
static_assert(std::nextafter(1.0f128, 2.0f128) == 1.0f128 + 0x1p-112f128);
static_assert(std::ldexp(1.0f128, -16494) == 0x1p-16494f128);
#endif
int main() {}
