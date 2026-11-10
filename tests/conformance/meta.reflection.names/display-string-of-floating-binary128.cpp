// [meta.reflection.names]/6-7 (P2996R13): a binary128 `long double`
// (aarch64) prints with GCC 16.1's 36 significant digits and the `l`
// suffix (spec/reflection.md §3.9, WP-1229). Host-dependent: no --target.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ requires: target-aarch64
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include <meta>
#include <string_view>
using namespace std::meta;

template <auto V> struct W {};

static_assert(display_string_of(reflect_constant(0.1L)) == "1.00000000000000000000000000000000005e-1l");
static_assert(display_string_of(reflect_constant(0.3L)) == "2.9999999999999999999999999999999999e-1l");
static_assert(display_string_of(reflect_constant(-1.0e4000L)) == "-1.00000000000000000000000000000000004e+4000l");
static_assert(display_string_of(reflect_constant(-0x1p16000L)) == "-3.01946933723922757953065844661527971e+4816l");
static_assert(display_string_of(^^W<0.1L>) == "W<1.00000000000000000000000000000000005e-1l>");

int main() {}
