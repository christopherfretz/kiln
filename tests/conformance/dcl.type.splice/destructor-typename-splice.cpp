// [class.dtor]/16, [dcl.type.splice]: a splice-type-specifier is a
// computed-type-specifier, so `x.~typename [: r :]()` names the destructor
// of the type r reflects, as `x.~decltype(x)()` does; also through a
// pointer, for a class template specialization and in a template. Found
// running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
#include <new>
struct S { int v; constexpr ~S() {} };
template <class T> struct W { T t; };
consteval int f() {
  S s{1};
  s.~typename [:^^S:]();
  std::construct_at(&s, 2);
  S *p = &s;
  p->~typename [:^^S:]();
  std::construct_at(p, 3);
  W<int> w{4};
  w.~typename [:^^W<int>:]();
  std::construct_at(&w, 5);
  return s.v + w.t;
}
static_assert(f() == 8);
template <class T> constexpr int g() {
  T t{6};
  t.~typename [:^^T:]();
  std::construct_at(&t, 7);
  return t.v;
}
static_assert(g<S>() == 7);
