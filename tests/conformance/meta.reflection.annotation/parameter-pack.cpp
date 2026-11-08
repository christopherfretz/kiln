// [dcl.attr.annotation]/3, [temp.variadic]/5, [meta.reflection.annotation]
// (P3394R4): an annotation on a function parameter pack appertains to each
// parameter the pack expands to, as a distinct annotation per element
// (parameters around the pack keep their own); a dependent operand is
// substituted for every element.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle"; WP-830).
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
template <class... T> void f([[=1]] T... xs) {}
template void f<int, long>(int, long);
static_assert(annotations_of(parameters_of(^^f<int, long>)[0]).size() == 1);
static_assert(annotations_of(parameters_of(^^f<int, long>)[1]).size() == 1);
static_assert(annotations_of(parameters_of(^^f<int, long>)[0])[0]
              != annotations_of(parameters_of(^^f<int, long>)[1])[0]);
static_assert(extract<int>(annotations_of(parameters_of(^^f<int, long>)[1])[0]) == 1);
template <class U, class... T> void g(char, [[=2]] U, [[='p']] T..., [[=3]] int) {}
template void g<short, char, double>(char, short, char, double, int);
static_assert(parameters_of(^^g<short, char, double>).size() == 5);
static_assert(annotations_of(parameters_of(^^g<short, char, double>)[0]).size() == 0);
static_assert(extract<int>(annotations_of(parameters_of(^^g<short, char, double>)[1])[0]) == 2);
static_assert(extract<char>(annotations_of(parameters_of(^^g<short, char, double>)[2])[0]) == 'p');
static_assert(extract<char>(annotations_of(parameters_of(^^g<short, char, double>)[3])[0]) == 'p');
static_assert(extract<int>(annotations_of(parameters_of(^^g<short, char, double>)[4])[0]) == 3);
template <class U, class... T> void k([[=sizeof(U)]] T... xs) {}
template void k<long, int, int>(int, int);
static_assert(extract<unsigned long>(annotations_of(parameters_of(^^k<long, int, int>)[1])[0]) == sizeof(long));
