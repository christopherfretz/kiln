// [dcl.type.decltype], [temp.type]/4: `decltype(e)` of a non-dependent
// operand and of a type-dependent one, as a return type (with SFINAE,
// [temp.deduct]/8), a member typedef and an alias template. With
// `--pch-diff`, the header's snapshot carries each operand (WP-1300).
//@ expect: run
#include "Inputs/expr_decltype.h"

struct Sz { using size_type = int; };
static_assert(sizeof(dt_long()) == sizeof(long));
static_assert(sizeof(dt_add(1, 2.0)) == sizeof(double));
static_assert(sizeof(dt_box<char>::type) == sizeof(int));
static_assert(sizeof(dt_t<short>) == sizeof(int));

int main() {
  bool ok = dt_has_size<Sz>(0) && !dt_has_size<int>(0);
  return ok && dt_long() == 5 && dt_add(2, 3) == 5 ? 0 : 1;
}
