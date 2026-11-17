// [expr.const] (CWG3162), [meta.reflection.queries] variable_of: a
// constant expression nested in an immediate function (a constexpr
// variable's initializer, a splice) may use variable_of of the enclosing
// function's own parameter, which is the function's current one. Found
// running GCC's C++ testsuite.
//@ xfail(kiln): WP-1614: nested constant evaluations (CWG3162) do not see the enclosing call's parameters
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
consteval int fn(int p) {
  constexpr auto rv = std::meta::variable_of(std::meta::parameters_of(^^fn)[0]);
  static_assert(rv == ^^p);
  return [:std::meta::variable_of(std::meta::parameters_of(^^fn)[0]):] + 1;
}
static_assert(fn(41) == 42);
