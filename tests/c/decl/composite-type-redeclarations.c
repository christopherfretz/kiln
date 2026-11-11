// C23 6.2.7p3 (composite type), 6.9.3p2: compatible redeclarations
// merge into the composite type (an array takes its size from either
// declaration, a prototype completes an old-style one, a pointer-to-array
// parameter its bound); `extern` followed by a definition, and repeated
// tentative definitions with one initializer, define one object.
// gcc.c-torture: compile/20000120-2.c, compile/990928-1.c.
//@ expect: run
//@ stdout: 3 4 2 1 5 12 3
#include <stdio.h>
int a;
int a;
int a = 3;
int a;
extern int b;
int b = 4;
extern int c[];
int c[3] = {1, 2, 3};
int f();
int f(int x);
int f(int x) { return x; }
int g(int (*)[]);
int g(int (*p)[3]) { return sizeof *p / sizeof **p; }
static int s;
static int s = 5;
extern int s;
int sz = sizeof(c);
void h(void) {
  extern int a;
  (void)a;
}
int main(void) {
  int arr[3];
  printf("%d %d %d %d %d %d %d\n", a, b, c[1], f(1), s, sz, g(&arr));
  return 0;
}
