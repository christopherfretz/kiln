// C23 6.8.7.2p1 (the goto statement): a goto may jump past the declaration
// of an object with an initializer, as long as no variably modified type is
// in scope at the label. C++ forbids this ([stmt.dcl]p3).
// gcc.c-torture: compile/irreducible-loop.c, execute/20180921-1.c.
//@ std: c17
//@ expect: run
//@ stdout: 5
#include <stdio.h>
static int f(int n) {
  if (n)
    goto skip;
  int x = 10;
  n = x;
skip:
  return n;
}
int main(void) {
  printf("%d\n", f(5));
  return 0;
}
