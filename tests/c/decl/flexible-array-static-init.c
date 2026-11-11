// C23 6.7.2.1p20 (structure specifiers): a flexible array member is
// ignored by initialization in ISO C; GNU C lets a static object
// initialize it, and the object then holds the elements (clang
// `CheckFlexibleArrayInit`, -Wgnu-flexible-array-initializer).
// gcc.c-torture: compile/pr33382.c, execute/20010924-1.c.
//@ std: c17
//@ expect: run
//@ args: -Wno-gnu-flexible-array-initializer
//@ stdout: 3 10 30 120 hello 6 4
#include <stdio.h>
struct S { int n; int f[]; };
extern struct S s;
int get(int i) { return s.f[i]; }
struct S s = {3, {10, 20, 30}};
struct C { int n; char c; char t[]; } c = {7, 'x', "hello"};
int main(void) {
  static struct S l = {2, {5, 6}};
  printf("%d %d %d %d %s %d %d\n", s.n, get(0), get(2), c.c, c.t, l.f[1], (int)sizeof s);
  return 0;
}
