// [complex.syn], [stdfloat.syn], [complex.transcendentals]: on x86-64
// `_Float128` is not `long double`, so libstdc++ 14's <complex> takes its
// `_GLIBCXX_HAVE_FLOAT128_MATH` branch and implements
// `std::complex<std::float128_t>` with GCC's `__builtin_c*f128` family,
// which kiln declares (WP-365; ext.complex/complex-f128-builtins.cpp).
// Where `long double` is binary128 (aarch64) the header uses the `*l`
// builtins instead, already covered by libstdcxx-complex-bf16.cpp.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ requires: target-x86_64
//@ skip(clang): clang 19/22 have no _Float128 in C++ and skip the block
#include <complex>
#include <stdfloat>

using C = std::complex<std::float128_t>;
static_assert(sizeof(C) == 32 && alignof(C) == 16);

C all(C z) {
  std::float128_t r = std::abs(z) + std::arg(z) + std::norm(z);
  return std::exp(z) + std::log(z) + std::pow(z, z) + std::sqrt(z) + std::sin(z) +
         std::sinh(z) + std::cos(z) + std::cosh(z) + std::tan(z) + std::tanh(z) +
         std::asin(z) + std::asinh(z) + std::acos(z) + std::acosh(z) + std::atan(z) +
         std::atanh(z) + std::proj(z) + std::polar(r, r);
}
