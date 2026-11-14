// [temp.arg.general]/9, [temp.dep.type]/9, [class.derived.general]/2: a
// class template named with dependent arguments that leave a default to
// fill, used as a type and through a member type as a base (the shape of
// <type_traits> `is_signed`). With `--pch-diff`, the header's
// snapshot carries the specialization with its defaults
// (WP-1320).
//@ expect: run
#include "Inputs/dependent_default_tst.h"

struct S {};

int main() {
  if (!ddt::is_signed<int>::value || !ddt::is_signed<long>::value)
    return 1;
  if (ddt::is_signed<S>::value)
    return 2;
  ddt::holder<int> h;
  if (sizeof(h.bytes) != sizeof(int) || sizeof(ddt::holder<S>::pair) != 2)
    return 3;
  return 0;
}
