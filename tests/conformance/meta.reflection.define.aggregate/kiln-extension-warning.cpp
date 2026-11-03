// spec/reflection.md §6.3: `__kiln_define_member` (the pre-P2996 spike
// vehicle) shares define_aggregate's injection path and warns under
// -Wkiln-extension (on by default; -Wno-kiln-extension silences it) at
// each call that defines; a trial that fails silently does not warn. A
// duplicate member injected by an earlier call points at the injection.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 26:15 warning "'__kiln_define_member' is a kiln extension; use 'std::meta::define_aggregate'"
//@ diag: 28:15 warning "'__kiln_define_member' is a kiln extension; use 'std::meta::define_aggregate'"
//@ diag: 28:3 error "consteval block is not a constant expression"
//@ diag: 28:15 note "duplicate member 'x'"
//@ diag: 28:3 note "in call to"
//@ diag: 26:15 note "previous declaration is here"
//@ diag: 26:3 note "member injected here"
//@ diag: 24:8 note "in evaluation of '__kiln_define_member' for class 'S' being defined here"
#include "kiln_meta.h"
using namespace std::meta;
template <class = void> constexpr bool try_define(info cls) {
  if consteval { return __kiln_define_member(cls, ^^int, "t") != info{}; }
  return false;
}
struct S {
  int a;
  consteval { __kiln_define_member(^^S, ^^int, "x"); }
  static_assert(!__builtin_constant_p(try_define(^^S)));
  consteval { __kiln_define_member(^^S, ^^long, "x"); }
};
static_assert(sizeof(S) == 2 * sizeof(int));
