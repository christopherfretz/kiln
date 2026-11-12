// [class.mem.general]/7, [class.pre]/2, [class.default.ctor]/1,
// [expr.prim.req.general], [expr.prim.id.general]/4, [dcl.spec.auto]:
// a noexcept operand naming a later member, a class named in its own
// base clause, an implicit default constructor named in a later type,
// a requires-expression of a checked concept, a data member in an
// unevaluated operand of a member's type, a function template
// declared with a deduced return type before its definition, and a
// member function template of a specialization in a dependent call. With
// `--pch-diff`, the header's precompiled image carries them (WP-1360).
//@ expect: run
#include "Inputs/outside_refs.h"

int main() {
  int x = 5;
  o1::counted<int> c{&x, 3};
  static_assert(noexcept(c.get()) && noexcept(c.left()));
  static_assert(noexcept(o1::counted<long>{}.left()));
  o1::plain p{4};
  if (c.get() + c.left() + p.get() + o1::made() != 13)
    return 1;
  static_assert(o2::derived{}.twice() == 42 && o2::both{}.twice() == 4);
  static_assert(o3::fn::go(1, 2).v && !o3::fn::go(2, 1).v);
  static_assert(!noexcept(o3::fn::go(1, 2)));
  static_assert(o4::testable<long> && !o4::conv<o3::cmp, bool>);
  static_assert(sizeof(o4::holder::buf) == sizeof(void *));
  static constexpr int arr[3] = {};
  static_assert(o5::span(arr, arr + 3) == 3);
  if (o5::span(arr, arr + 2) != 2)
    return 3;
  return o4::via(2.5) == 3 ? 0 : 2;
}
