// [expr.ref]/7, [over.match.call]/2, [over.match.funcs]/4,
// [namespace.udecl]/14, [class.member.lookup], [temp.arg.explicit]/1:
// an overloaded member access in a dependent call (implicit `this`,
// `.` and `->`, a static candidate, explicit template arguments, a
// hidden base member, a using-declaration, a class template's
// `this->f`) chooses at each instantiation the overload the source
// does. With `--pch-diff`, the header's snapshot carries the
// whole candidate set (WP-1368).
//@ expect: run
#include "Inputs/ovl_members.h"

using namespace om;
static_assert(dist_fn{}(vec{}) == 4 && dist_fn{}.twice(vec{}) == 8);
static_assert(dot(1) == 1 && dot(1L) == 2 && dot(1.0) == 3);
static_assert(arrow(1) == 1 && arrow(1L) == 2 && arrow(1.0) == 3);
static_assert(targs(1) == 30 && targs(1L) == 20);
static_assert(hid(1) == 2 && hid("x") == 5);
static_assert(used(1) == 1 && used('c') == 4 && used(1L) == 3);
static_assert(gen<int>{5}.run(2) == 7 && callp(1L) == 1);

int main() {
  vec v{};
  gen<long> g{1};
  if (dist_fn{}(v) + dist_fn{}.twice(v) != 12)
    return 1;
  if (dot(1L) + arrow(1.0) + targs(1) + hid(1) + used(1.5) != 43)
    return 2;
  return g.run(1) == 2 ? 0 : 3;
}
