// C23 6.9.2p12 (function definitions): reaching the `}` of a function
// whose return type is not void is undefined only when the caller uses the
// value. Here no caller does, so the program is valid and prints 5 2; kiln
// applies the C++ rule ([stmt.return]p4) and traps (-O0) or treats the end
// as unreachable (-O2). Miscompile: 36 gcc.c-torture execute cases
// (20000717-1.c, compare-1.c, ieee/compare-fp-1.c, pr28403.c, ...).
//@ std: c17
//@ expect: run
//@ stdout: 5 2
//@ xfail(kiln): WP-1282 follow-up, miscompile: falling off a non-void C function traps or is unreachable (the C++ rule)
#include <stdio.h>
static int g;
static int set(int v) { g = v; }
static int check(int v) {
  if (v < 0)
    return -1;
}
int main(void) {
  set(5);
  check(1);
  printf("%d %d\n", g, check(-2) + 3);
  return 0;
}
