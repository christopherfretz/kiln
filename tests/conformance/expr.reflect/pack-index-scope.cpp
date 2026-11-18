// [expr.reflect]/1, [dcl.type.pack.index]: a pack-index-specifier is a
// type, so it can open the nested-name-specifier of a reflection-name
// (`^^Ts...[0]::sx`) and of a typename-specifier in a type-id
// (`^^typename Ts...[0]::type`). Found running GCC's C++ testsuite;
// GCC 16.1 accepts both.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
#include <meta>
using namespace std::meta;

struct S {
  static int sx;
  using type = long;
};
struct U {
  static int sx;
  using type = short;
};

template <class... Ts>
consteval bool f() {
  constexpr auto r1 = ^^Ts...[1]::sx;
  constexpr auto r2 = ^^typename Ts...[0]::type;
  return (r1 == ^^U::sx) && (r2 == ^^S::type) && (dealias(r2) == ^^long) && is_variable(r1);
}
static_assert(f<S, U>());
