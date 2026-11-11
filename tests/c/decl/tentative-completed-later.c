// C23 6.9.3p2 (external object definitions): tentative definitions of an
// object may be repeated and may precede the completion of its type or a
// definition with an initializer; at the end of the translation unit the
// object is defined with the composite (now complete) type, zero-initialized.
// gcc.c-torture: compile/20000120-2.c, execute/pr43191.c.
//@ args: -Wno-tentative-definition-array
//@ expect: run
//@ stdout: 20 8 24 1 2 7 3 4
#include <stdio.h>
int a[];
int b[];
int b[5];
int c = 7;
int c;
int d;
int d;
struct U u;
struct U { int y, z; };
static struct S s;
struct S { long q[3]; };
int *pa = a;
int main(void) {
  a[0] = 1;
  b[4] = 2;
  u.z = 3;
  s.q[2] = 4;
  printf("%zu %zu %zu %d %d %d %d %d\n", sizeof b, sizeof u, sizeof s, a[0], b[4], c, u.z,
         (int)s.q[2] + d);
  return 0;
}
