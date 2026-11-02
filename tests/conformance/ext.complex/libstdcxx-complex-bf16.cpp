// [complex.syn], [stdfloat.syn]: libstdc++ 14 <complex> declares
// `__complex__ decltype(0.0bf16)` and specializes std::complex for
// std::bfloat16_t when the compiler defines __STDCPP_BFLOAT16_T__ (kiln
// predefines it as GCC does), so `_Complex __bf16` must be a valid type
// for <complex> (and <ccomplex>, <ctgmath>) to compile at all (WP-365).
// clang 19/22 do not define the macro and skip the block. (std::complex's
// `+=`, used by `+`, is not a constant expression in clang 19 or kiln:
// spec/oven.md "Complex values".)
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
#include <complex>
#include <ccomplex>
#include <ctgmath>
#ifdef __STDCPP_BFLOAT16_T__
#include <stdfloat>
using C = std::complex<std::bfloat16_t>;
static_assert(sizeof(C) == 4 && alignof(C) == 2);
constexpr C a(1.5bf16, 2.0bf16), b(2.0bf16, -1.0bf16);
static_assert(a.real() == 1.5bf16 && std::conj(a).imag() == -2.0bf16);
static_assert(__real__ (a.__rep() * b.__rep()) == 5.0bf16);
C mul(C x, C y) { return x * y; }
#endif
std::complex<double> z(1.0, 2.0);
