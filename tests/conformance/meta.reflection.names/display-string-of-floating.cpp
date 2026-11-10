// [meta.reflection.names]/6-7 (P2996R13), [temp.arg.nontype]: the
// display_string_of of a floating-point constant (a template argument,
// reflect_constant, a member of a class value) and of a specialization
// with one as an argument is GCC 16.1's spelling (spec/reflection.md §3.9,
// WP-1229): `pp_c_floating_constant`, i.e. the format's max_digits10
// significant digits (9 for float, 17 for double) with trailing zeros
// cropped but one kept after the point, a signed exponent, and the
// literal suffix of the type (`f`, `l`, `f32`...; none for double); a zero
// as `0.0`, an infinity as `+Inf`, a NaN as `+QNaN`. So a float and a
// double argument of the same value name different specializations.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include <meta>
#include <limits>
#include <stdfloat>
#include <string_view>
using namespace std::meta;

template <auto V> struct W {};
template <class T, auto Tolerance> struct Tol {};

consteval std::string_view arg0(info r) {
  return display_string_of(template_arguments_of(r)[0]);
}

// The value, its type's suffix, and the specialization's name.
static_assert(display_string_of(template_arguments_of(^^Tol<int, 1.0f>)[1]) == "1.0e+0f");
static_assert(display_string_of(template_arguments_of(^^Tol<int, 1.0>)[1]) == "1.0e+0");
static_assert(display_string_of(^^Tol<int, 1.0f>) == "Tol<int, 1.0e+0f>");
static_assert(display_string_of(^^Tol<int, 1.0>) == "Tol<int, 1.0e+0>");
static_assert(display_string_of(^^Tol<int, 1.0f>) != display_string_of(^^Tol<int, 1.0>));
static_assert(display_string_of(reflect_constant(1.0)) == "1.0e+0");
static_assert(display_string_of(reflect_constant(2.5f)) == "2.5e+0f");
static_assert(arg0(^^W<1.0L>) == "1.0e+0l");
static_assert(arg0(^^W<2.5L>) == "2.5e+0l");
static_assert(display_string_of(^^W<2.5L>) == "W<2.5e+0l>");

// Signs, zeros, digits, exponents.
static_assert(arg0(^^W<-1.0>) == "-1.0e+0");
static_assert(arg0(^^W<-2.5f>) == "-2.5e+0f");
static_assert(arg0(^^W<0.0>) == "0.0");
static_assert(arg0(^^W<-0.0>) == "-0.0");
static_assert(arg0(^^W<0.0f>) == "0.0f");
static_assert(arg0(^^W<-0.0f>) == "-0.0f");
static_assert(arg0(^^W<0.0L>) == "0.0l");
static_assert(display_string_of(^^W<-0.0>) == "W<-0.0>");
static_assert(arg0(^^W<0.1>) == "1.0000000000000001e-1");
static_assert(arg0(^^W<0.1f>) == "1.00000001e-1f");
static_assert(arg0(^^W<1e100>) == "1.0e+100");
static_assert(arg0(^^W<1e-100>) == "1.0e-100");
static_assert(arg0(^^W<1e30f>) == "1.00000002e+30f");
static_assert(arg0(^^W<1e-30f>) == "1.0e-30f");
static_assert(arg0(^^W<123456.0>) == "1.23456e+5");
static_assert(arg0(^^W<1.5e300>) == "1.5000000000000001e+300");
static_assert(arg0(^^W<3.0>) == "3.0e+0");
static_assert(arg0(^^W<100.0>) == "1.0e+2");
static_assert(arg0(^^W<1e-4>) == "1.0e-4");
static_assert(arg0(^^W<1e23>) == "9.9999999999999992e+22");
static_assert(arg0(^^W<5e-324>) == "4.9406564584124654e-324");
static_assert(arg0(^^W<1.7976931348623157e308>) == "1.7976931348623157e+308");
static_assert(arg0(^^W<std::numeric_limits<float>::denorm_min()>) == "1.40129846e-45f");
static_assert(display_string_of(^^W<0.1f>) == "W<1.00000001e-1f>");

// No finite value: GCC's real_to_decimal words, then the suffix.
static_assert(arg0(^^W<std::numeric_limits<double>::infinity()>) == "+Inf");
static_assert(arg0(^^W<-std::numeric_limits<float>::infinity()>) == "-Inff");
static_assert(arg0(^^W<std::numeric_limits<double>::quiet_NaN()>) == "+QNaN");
static_assert(display_string_of(^^W<std::numeric_limits<double>::infinity()>) == "W<+Inf>");

// The extended floating-point types' suffixes (std::bfloat16_t has none).
static_assert(arg0(^^W<1.5f16>) == "1.5e+0f16");
static_assert(arg0(^^W<1.5f32>) == "1.5e+0f32");
static_assert(arg0(^^W<1.5f64>) == "1.5e+0f64");
static_assert(arg0(^^W<1.5f128>) == "1.5e+0f128");
static_assert(arg0(^^W<1.5bf16>) == "1.5e+0");
static_assert(display_string_of(^^W<1.5f32>) == "W<1.5e+0f32>");

// Inside a class value and a nested reflection.
struct FA {
  float f[2];
  double d;
};
static_assert(arg0(^^W<FA{{1.0f, 2.5f}, 0.5}>) == "FA{float [2]{1.0e+0f, 2.5e+0f}, 5.0e-1}");
static_assert(arg0(^^W<reflect_constant(1.5f)>) == "^^1.5e+0f");

int main() {}
