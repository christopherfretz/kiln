// [expr.reflect]/5 [temp.inst] P2996R13: in a template, `^^` of a member
// or a template parameter reflects the instantiated entity: each
// specialization has its own members, and `^^T` is the argument type.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
using I = int;
template <class T> struct B {
  int m;
  static int s;
  static constexpr info rm = ^^m;
  static constexpr info rs = ^^s;
  static constexpr info rt = ^^T;
  static constexpr info rb = ^^B;
};
static_assert(B<int>::rm == ^^B<int>::m);
static_assert(B<int>::rm != B<long>::rm);
static_assert(B<int>::rs == ^^B<int>::s);
static_assert(B<int>::rt == ^^int);
static_assert(B<I>::rt == ^^int);  // the argument, dealiased
static_assert(B<long>::rt == ^^long);
static_assert(B<int>::rb == ^^B<int>);
template <class T> consteval info local() { T v{}; return ^^v; }
static_assert(local<int>() != local<long>());
