// [meta.reflection.result]/7-8, [temp.arg.nontype]/4 (P2996R13):
// reflect_constant(V) of a class type throws when the template parameter
// object copy-initialized from V would not be template-argument-equivalent
// to V (a copy constructor that changes the value). Found running GCC's
// C++ testsuite.
//@ xfail(kiln): WP-1614: reflect_constant does not check that the copy is template-argument-equivalent
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
struct A {
  int x;
  constexpr A(int p) : x(p) {}
  constexpr A(const A &o) : x(o.x ? o.x - 1 : o.x) {}
};
consteval bool ok(int x) {
  try {
    (void)std::meta::reflect_constant(A(x));
  } catch (std::meta::exception &) {
    return false;
  }
  return true;
}
static_assert(!ok(4));
