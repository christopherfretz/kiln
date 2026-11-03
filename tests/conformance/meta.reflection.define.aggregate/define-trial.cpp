// [meta.reflection.define.aggregate]/7 (P2996R13), [expr.const]: a trial
// evaluation (the operand of __builtin_constant_p, a fold) never defines
// the class: the call fails silently (the operand is then not constant)
// and the class stays incomplete; the same call in a consteval block
// defines it.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

struct T;
template <class = void> constexpr bool try_define(info cls) {
  if consteval {
    define_aggregate(cls, {data_member_spec(^^int, {.name = "x"})});
    return true;
  }
  return false;
}
static_assert(!__builtin_constant_p(try_define(^^T)));
consteval {
  if (__builtin_constant_p(try_define(^^T))) throw 0;
}
// Still incomplete: its members cannot be counted.
template <class = void> constexpr bool complete(info cls) {
  if consteval { return __kiln_members_count(cls) < 100; }
  return false;
}
static_assert(!__builtin_constant_p(complete(^^T)));
consteval { try_define(^^T); }
static_assert(sizeof(T) == sizeof(int));
static_assert(__kiln_members_count(^^T) == 1);
static_assert(__builtin_constant_p(complete(^^T)));
