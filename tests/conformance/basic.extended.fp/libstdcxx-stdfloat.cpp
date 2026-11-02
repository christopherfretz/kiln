// [stdfloat.syn], [basic.extended.fp], [numeric.limits], [meta.trans.other]
// (C++23, P1467R9): libstdc++'s <stdfloat> defines std::float16_t,
// float32_t, float64_t, float128_t and bfloat16_t when the compiler defines
// __STDCPP_FLOAT16_T__ ... __STDCPP_BFLOAT16_T__; <limits> specializes
// numeric_limits from the __FLT32_*__ ... __BFLT16_*__ macros and the
// __builtin_{huge_val,inf,nan,nans}fN builtins, <cmath> adds the overloads
// and std::common_type follows the conversion ranks. libc++ 19 has no
// <stdfloat>.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(clang): clang 19/22 do not define __STDCPP_FLOAT32_T__ etc., so <stdfloat> declares nothing
#include <stdfloat>
#include <limits>
#include <cmath>
#include <charconv>
#include <type_traits>
#include <concepts>
static_assert(std::is_same_v<std::float16_t, _Float16> && std::is_same_v<std::float32_t, _Float32>);
static_assert(std::is_same_v<std::float64_t, _Float64> && std::is_same_v<std::float128_t, _Float128>);
static_assert(std::is_same_v<std::bfloat16_t, __bf16> && std::is_same_v<std::bfloat16_t, decltype(0.0bf16)>);
static_assert(!std::is_same_v<std::float64_t, double> && !std::is_same_v<std::float32_t, float>);
static_assert(std::is_floating_point_v<std::bfloat16_t> && std::floating_point<std::float128_t>);

static_assert(std::numeric_limits<std::float32_t>::digits == 24);
static_assert(std::numeric_limits<std::float64_t>::max() == 0x1.fffffffffffffp1023f64);
static_assert(std::numeric_limits<std::float16_t>::infinity() > 65504.0f16);
static_assert(std::numeric_limits<std::bfloat16_t>::digits == 8);
static_assert(std::numeric_limits<std::bfloat16_t>::max_digits10 == 4);
static_assert(std::numeric_limits<std::bfloat16_t>::epsilon() == 0x1p-7bf16);
static_assert(std::numeric_limits<std::float128_t>::digits == 113);
static_assert(std::numeric_limits<std::float128_t>::max() == __FLT128_MAX__);
static_assert(std::numeric_limits<std::float32_t>::quiet_NaN() != std::numeric_limits<std::float32_t>::quiet_NaN());
static_assert(std::numeric_limits<std::bfloat16_t>::has_signaling_NaN);

// [meta.trans.other]: the usual arithmetic conversions decide common_type.
static_assert(std::is_same_v<std::common_type_t<std::float32_t, float>, std::float32_t>);
static_assert(std::is_same_v<std::common_type_t<std::float32_t, double>, double>);
static_assert(std::is_same_v<std::common_type_t<std::float64_t, std::float128_t>, std::float128_t>);
static_assert(std::is_same_v<std::common_type_t<std::bfloat16_t, float>, float>);
template<class T, class U> concept has_common = requires { typename std::common_type_t<T, U>; };
static_assert(!has_common<std::float16_t, std::bfloat16_t>); // unordered ranks

std::float32_t f(std::float32_t x) { return std::sqrt(x) + std::fabs(x); }
std::float128_t g(std::float128_t x) { return std::fma(x, x, x); }
std::bfloat16_t h(std::bfloat16_t x) { return std::sqrt(x); }
std::float16_t k(std::float16_t x) { return std::nextafter(x, std::float16_t(2)); }
bool fin(std::float64_t x) { return std::isfinite(x); }
std::to_chars_result tc(char *b, char *e, std::float32_t x) { return std::to_chars(b, e, x); }
