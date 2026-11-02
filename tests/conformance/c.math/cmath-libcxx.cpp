// [c.math.syn], [c.math.fpclass] (P0533 through libc++'s <cmath>): libc++ 19
// makes isfinite/isinf/isnan/isnormal constexpr since C++23 (for floating
// and integral arguments) over __builtin_isfinite & co.; libc++ 22 adds
// signbit over __builtin_signbit. Neither defines __cpp_lib_constexpr_cmath
// or marks the rest of the P0533 set constexpr yet; those builtins are
// covered directly by the constexpr-*.cpp tests here.
//@ expect: compile-ok
//@ strict-diags
//@ args: -stdlib=libc++
//@ std: c++23
#include <cmath>

static_assert(std::isfinite(1.0) && !std::isfinite(HUGE_VAL) && std::isfinite(3));
static_assert(std::isinf(-HUGE_VALF) && !std::isinf(1.0L) && !std::isinf(7));
static_assert(std::isnan(NAN) && !std::isnan(0.0) && !std::isnan(2LL));
static_assert(std::isnormal(1.0f) && !std::isnormal(0x1p-1074) && !std::isnormal(0.0L) && std::isnormal(5));
#if _LIBCPP_VERSION >= 220000
static_assert(std::signbit(-0.0) && !std::signbit(1.0f) && std::signbit(-1.0L) && !std::signbit(3));
#endif
int main() {}
