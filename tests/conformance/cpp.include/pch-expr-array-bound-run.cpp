// [temp.dep.constexpr], [dcl.array]: array bounds that are
// value-dependent expressions, in a member, a parameter (with deduction
// through `N + 1` not deducible, [temp.deduct.type]/5) and an alias
// template. With `--pch-diff`, the header's snapshot carries each bound
// (WP-1300).
//@ expect: run
#include "Inputs/expr_array_bound.h"

static_assert(sizeof(ab_buf<short>::data) == 3 * sizeof(short));
static_assert(sizeof(ab_twice<3>) == 6);
static_assert(sizeof(ab_arr<int>) == sizeof(int) * sizeof(int));

int main() {
  int a[3] = {};
  ab_buf<char> b{};
  return ab_count<2>(a) == 2 && sizeof b.data == 2 ? 0 : 1;
}
