// [class.mem]/1, [cpp.include]: a builtin function first used inside a
// class definition of an included header is the global builtin. With
// `--pch-diff`, the header's snapshot carries the class with
// the builtin's record among its members (WP-1310: before, the snapshot's
// own check refused it as an internal inconsistency).
//@ expect: run
#include "Inputs/builtin_in_class.h"

int main() {
  BuiltinUser u{5};
  if (u.f(-3) != 3 || u.g != 5)
    return 1;
  if (!(BuiltinUser::inf() > 1e308))
    return 2;
  return __builtin_abs(0);
}
