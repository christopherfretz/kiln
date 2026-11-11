// [dcl.asm] GNU asm labels: a label given on a redeclaration names the
// entity's symbol, so calls through the earlier prototype reach it (the
// glibc `__REDIRECT` shape in <stdio.h> and <inttypes.h>). With
// `--pch-diff`, the header's precompiled image carries the label on its
// redeclaration (WP-1310).
//@ expect: run
#include "Inputs/redirect_label.h"

extern "C" int redirect_target_add(int a, int b) { return a + b; }
extern "C" int redirect_target_twice(int a) { return 2 * a; }

int main() {
  if (redirected_add(2, 3) != 5 || rl::twice(4) != 8)
    return 1;
  return rl::both(5) == 16 ? 0 : 2;
}
