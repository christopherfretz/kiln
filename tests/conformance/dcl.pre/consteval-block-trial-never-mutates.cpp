// [expr.const.core]/2.32, [dcl.pre]/P3289R1 (spec/consteval-blocks.md
// §3): a trial evaluation (the operand of __builtin_constant_p, a fold)
// never injects: the `__kiln_define_member` call in it fails silently (the
// operand is then not constant) and the class is unchanged; the same call
// in a consteval block adds the member.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 has no consteval blocks (P3289R1) or reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
consteval bool define(info cls, const char* name) {
  return __kiln_define_member(cls, ^^int, name) != info{};
}
// A templated constexpr function (it takes a consteval-only parameter).
template <class = void> constexpr bool try_define(info cls) {
  if consteval { return define(cls, "x"); }
  return false;
}
struct T {
  int a;
  static_assert(!__builtin_constant_p(try_define(^^T)));
  consteval {
    if (__builtin_constant_p(try_define(^^T))) throw 0;
  }
};
static_assert(sizeof(T) == sizeof(int));
struct U {
  int a;
  consteval { try_define(^^U); }
};
static_assert(sizeof(U) == 2 * sizeof(int));
static_assert(__builtin_offsetof(U, x) == sizeof(int));
