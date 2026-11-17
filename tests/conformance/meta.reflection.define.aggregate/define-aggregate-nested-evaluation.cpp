// [expr.const] (CWG3162), [meta.reflection.define.aggregate]: a constexpr
// variable's initializer inside an immediate function is its own
// evaluation, so is_complete_type there sees the class incomplete before
// the enclosing call defines it, in every recursive call. Found running
// GCC's C++ testsuite.
//@ xfail(kiln): WP-1614: nested constant evaluations (CWG3162) see the enclosing evaluation's injected declarations
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
struct S;
consteval int foo(int p) {
  constexpr int r = std::meta::is_complete_type(^^S) ? 1 : 2;
  if (!std::meta::is_complete_type(^^S))
    std::meta::define_aggregate(^^S, {});
  return p > 0 ? foo(p - 1) : r;
}
consteval {
  if (foo(1) != 2)
    throw 0;
}
