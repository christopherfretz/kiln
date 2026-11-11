// C23 6.7.11p4, 6.6p9 (GNU labels as values): the difference of two label
// addresses (`&&a - &&b`) in an initializer of an object with static
// storage duration is a constant, also converted to a narrower integer
// (clang's `AddrLabelDiff`; gcc.c-torture compile/labels-3.c), in an array,
// a structure and a scalar. clang 22 no longer narrows one below 32 bits
// (tests/c/decl/static-init-not-constant-22.c).
//@ requires: clang<22
//@ args: -w
//@ expect: run
//@ stdout: 1 2 2 2
#include <stdio.h>
static int foo(int a) {
  static const short ar[] = { &&l1 - &&l1, &&l2 - &&l1 };
  static struct { int d; } s = { &&l2 - &&l1 };
  static long d = &&l2 - &&l1;
  void *p = &&l1 + (a == 0 ? ar[0] : a == 1 ? ar[1] : a == 2 ? s.d : d);
  goto *p;
l1:
  return 1;
l2:
  return 2;
}
int main(void) {
  printf("%d %d %d %d\n", foo(0), foo(1), foo(2), foo(3));
  return 0;
}
