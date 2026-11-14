// [dcl.pre]/10, [dcl.typedef]/2, [stmt.dcl]: block-scope static_assert
// declarations, typedefs and alias declarations in a header's function
// bodies. With `--pch-diff`, the header's snapshot carries them
// (WP-1320).
//@ expect: run
#include "Inputs/local_decls.h"

static_assert(lcd::sizes<int>() == 2 * int(sizeof(int)));
static_assert(lcd::small<char>());

int main() {
  int r = lcd::sizes<long>() + lcd::sizes<char>() + lcd::plain();
  if (lcd::small<long double>())
    return 1;
  return r == 2 * int(sizeof(long)) + 5 ? 0 : 2;
}
