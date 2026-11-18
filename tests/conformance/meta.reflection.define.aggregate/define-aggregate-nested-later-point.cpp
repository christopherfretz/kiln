// [expr.const.reflect] (CWG3162), [module.reach]: the evaluation context
// of a constexpr variable's initializer holds its point of evaluation,
// in the body of f, and the injected declarations produced during it. A
// class completed by an earlier consteval block is defined by a
// declaration whose locus follows that block, after f's body, so it is
// not reachable from there: is_complete_type is false in every call of
// f. GCC 16.1 agrees (it evaluates the initializer once, at f's
// definition).
//@ xfail(kiln): WP-1660 remainder: a nested evaluation's point of evaluation is not compared with the locus of a class's definition
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
struct S;
consteval bool f() {
  constexpr bool c = std::meta::is_complete_type(^^S);
  return c;
}
consteval { std::meta::define_aggregate(^^S, {}); }
static_assert(std::meta::is_complete_type(^^S));
static_assert(!f());
