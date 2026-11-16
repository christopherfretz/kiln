// [expr.reflect]/1: `^^ nested-name-specifier template identifier` is a
// reflection-name naming the template the identifier denotes, after any
// qualifier (a class or a namespace), for a class, function, operator
// function, variable and alias template. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
struct S {
  template <class T> struct B {};
  template <class T> void m(T) {}
  template <class T> S operator+(T) { return *this; }
  template <class T> static constexpr int v = 1;
};
namespace N { template <class T> void f(T) {} template <class T> using A = T*; }
constexpr auto r1 = ^^S::template B;
constexpr auto r2 = ^^S::template m;
constexpr auto r3 = ^^S::template operator+;
constexpr auto r4 = ^^S::template v;
constexpr auto r5 = ^^N::template f;
constexpr auto r6 = ^^N::template A;
static_assert(r1 == ^^S::B && r2 == ^^S::m && r5 == ^^N::f && r6 == ^^N::A);
static_assert(std::meta::is_template(r1) && std::meta::is_template(r3) && std::meta::is_template(r4));
template <class X> constexpr auto rd = ^^X::template B;
