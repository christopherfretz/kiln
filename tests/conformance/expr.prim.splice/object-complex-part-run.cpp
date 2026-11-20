// [expr.prim.splice]/2 (P2996R13) of an object reflection of the real or
// imaginary part of a GNU complex object: reflect_object takes an object
// suitable as a constant template argument of reference type, which kiln
// (as clang) allows for `__real__ z` / `__imag__ z`
// (temp.arg.nontype/complex-part-reference.cpp), so the splice is an
// lvalue naming that part: stores through it change the complex object
// and its address is the part's (WP-1743). GCC 16.1 rejects
// reflect_object of a complex part ("reflect_object failed"); see
// docs/status/deviations.md.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ stdout: 2.5 0 0 1.5
//@ stdout: 1 1 4
#include <meta>
#include <cstdio>
using namespace std::meta;
struct C { _Complex double z; };
constinit C gc{};
constinit _Complex float cf = 0;
constexpr _Complex double cz = 3.0 + 4.0i;
constexpr auto o_re = reflect_object(__real__ gc.z);
constexpr auto o_im = reflect_object(__imag__ cf);
constexpr auto o_cz = reflect_object(__imag__ cz);
static_assert([:o_cz:] == 4.0);
static_assert(type_of(o_re) == (^^double) && type_of(o_im) == (^^float));
int main() {
  [:o_re:] = 2.5;
  [:o_im:] = 1.5f;
  std::printf("%g %g %g %g\n", __real__ gc.z, __imag__ gc.z, (double)__real__ cf,
              (double)__imag__ cf);
  std::printf("%d %d %g\n", &[:o_re:] == &__real__ gc.z, &[:o_im:] == &__imag__ cf,
              [:o_cz:]);
}
