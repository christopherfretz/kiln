// [meta.reflection.names]/6-7 (P2996R13), [temp.arg.nontype]: the
// display_string_of of a value reflection (a constant template argument,
// reflect_constant) is the value's source spelling, and that of an object
// reflection (reflect_object, a reference or class-type template
// argument) the object's name, or for a template parameter object its
// value (spec/reflection.md §3.9, WP-564). Integers print in decimal
// without a suffix, characters as literals, enumerations by enumerator
// (else as a cast), class values as `T{...}`. Every row here prints the
// same in GCC 16.1 (spec/harness.md "GCC oracle"); the rows where kiln
// differs are in display-string-of-values-kiln.cpp.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

template <auto V> struct W {};
template <auto& R> struct WR {};
enum class Col { red = 2, green = 3 };
enum Plain { pa = 1 };
struct Pt {
  int x;
};
constexpr Pt pt{3};
int gv;
const int arr[3] = {1, 2, 3};
namespace ns {
int nv;
struct In {
  int m;
};
enum E { e1 = 4 };
enum class F : unsigned char { f = 1 };
}  // namespace ns
struct S {
  int m;
};

consteval std::string_view arg0(info r) {
  return display_string_of(template_arguments_of(r)[0]);
}

// Values of scalar type.
static_assert(arg0(^^W<42>) == "42");
static_assert(arg0(^^W<86400L>) == "86400");
static_assert(arg0(^^W<7UL>) == "7");
static_assert(arg0(^^W<-5>) == "-5");
static_assert(arg0(^^W<(short)5>) == "5");
static_assert(arg0(^^W<true>) == "true");
static_assert(arg0(^^W<false>) == "false");
static_assert(arg0(^^W<'a'>) == "'a'");
static_assert(arg0(^^W<nullptr>) == "nullptr");
static_assert(arg0(^^W<&S::m>) == "&S::m");
static_assert(arg0(^^W<^^int>) == "^^int");

// Enumerations: the enumerator, qualified as a scoped enumerator is and an
// unscoped one is not; a value naming none as a cast.
static_assert(arg0(^^W<Col::red>) == "Col::red");
static_assert(arg0(^^W<Col(5)>) == "(Col)5");
static_assert(arg0(^^W<pa>) == "pa");
static_assert(arg0(^^W<ns::e1>) == "ns::e1");
static_assert(arg0(^^W<ns::F::f>) == "ns::F::f");
static_assert(arg0(^^W<ns::F(9)>) == "(ns::F)9");

// A class-type argument is its template parameter object: the value.
static_assert(arg0(^^W<pt>) == "Pt{3}");
static_assert(arg0(^^W<ns::In{7}>) == "ns::In{7}");

// A reference argument is the object it binds.
static_assert(arg0(^^WR<gv>) == "gv");
static_assert(arg0(^^WR<ns::nv>) == "ns::nv");

// reflect_constant: a value (a class value is a template parameter
// object); reflect_object: the object's name or designator.
static_assert(display_string_of(reflect_constant(42)) == "42");
static_assert(display_string_of(reflect_constant(86400L)) == "86400");
static_assert(display_string_of(reflect_constant(Col::red)) == "Col::red");
static_assert(display_string_of(reflect_constant(pt)) == "Pt{3}");
static_assert(display_string_of(reflect_object(pt)) == "pt");
static_assert(display_string_of(reflect_object(gv)) == "gv");
static_assert(display_string_of(reflect_object(ns::nv)) == "ns::nv");
static_assert(display_string_of(reflect_object(arr[1])) == "arr[1]");

// A template-id round trip: the arguments spell back into the source.
template <long long N, long long D = 1> struct ratio {};
static_assert(arg0(^^ratio<86400, 7>) == "86400");
static_assert(display_string_of(template_arguments_of(^^ratio<86400, 7>)[1]) == "7");

// Values have no identifier ([meta.reflection.names]/1).
static_assert(!has_identifier(reflect_constant(42)));
static_assert(!has_identifier(template_arguments_of(^^W<pt>)[0]));

int main() {}
