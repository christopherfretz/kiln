// [dcl.attr.annotation]/3, [temp.variadic]/5, [meta.reflection.annotation]
// (P3394R4): with several function parameter packs (an explicitly
// specified one ahead of others, or a class template's pack ahead of a
// member template's), each pack's annotations appertain to exactly the
// parameters that pack expanded to.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle"; WP-866).
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;
template <class... T> struct L {};
template <class... A, class... B> void f(L<A...>, [[=1]] A... a, L<B...>, [[=2]] B... b) {}
template void f<int, long, char>(L<int, long, char>, int, long, char, L<>);
static_assert(parameters_of(^^f<int, long, char>).size() == 5);
static_assert(annotations_of(parameters_of(^^f<int, long, char>)[0]).size() == 0);
static_assert(extract<int>(annotations_of(parameters_of(^^f<int, long, char>)[1])[0]) == 1);
static_assert(extract<int>(annotations_of(parameters_of(^^f<int, long, char>)[3])[0]) == 1);
static_assert(annotations_of(parameters_of(^^f<int, long, char>)[3]).size() == 1);
static_assert(annotations_of(parameters_of(^^f<int, long, char>)[4]).size() == 0);
template <class U, class... A, class... B> void g([[=sizeof(U)]] A... a, char, [[=3]] B... b) {}
static_assert(parameters_of(^^g<double, int, short>).size() == 3);
static_assert(extract<unsigned long>(annotations_of(parameters_of(^^g<double, int, short>)[0])[0]) == sizeof(double));
static_assert(extract<unsigned long>(annotations_of(parameters_of(^^g<double, int, short>)[1])[0]) == sizeof(double));
static_assert(annotations_of(parameters_of(^^g<double, int, short>)[2]).size() == 0);
template <class... A> struct S {
  template <class... B> static void m([[=4]] A... a, [[=5]] B... b) {}
};
template void S<int, long>::m<char, short>(int, long, char, short);
static_assert(parameters_of(^^S<int, long>::m<char, short>).size() == 4);
static_assert(extract<int>(annotations_of(parameters_of(^^S<int, long>::m<char, short>)[0])[0]) == 4);
static_assert(extract<int>(annotations_of(parameters_of(^^S<int, long>::m<char, short>)[1])[0]) == 4);
static_assert(extract<int>(annotations_of(parameters_of(^^S<int, long>::m<char, short>)[2])[0]) == 5);
static_assert(extract<int>(annotations_of(parameters_of(^^S<int, long>::m<char, short>)[3])[0]) == 5);
static_assert(annotations_of(parameters_of(^^S<int, long>::m<char, short>)[3]).size() == 1);
