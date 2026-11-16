// [temp.dep.namespace], [namespace.alias]: in a template, `namespace A =
// [: R :];` with a dependent splice-specifier declares a dependent
// namespace alias, resolved at instantiation. Found running GCC's C++
// testsuite; GCC 16.1 accepts it.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ xfail(kiln): WP-1524: a namespace alias of a dependent splice-specifier is not instantiated
#include <meta>

namespace N { constexpr int v = 7; }

template <std::meta::info R> int get() {
  namespace A = [:R:];
  return A::v;
}

int main() { return get<^^N>() == 7 ? 0 : 1; }
