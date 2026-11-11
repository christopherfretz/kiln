// C has no class scope (C23 6.2.1p4, 6.7.2.1): an enumeration declared in
// a member list, named or not, and a named struct declared there belong
// to the scope enclosing the outermost structure, so their enumerators
// and tags are usable at file scope, or in the block, after it.
// gcc.c-torture: compile/pr37955.c.
//@ std: c17
//@ expect: run
//@ stdout: 4 7 3 22 5
#include <stdio.h>
struct S { enum { A0 = 3, A1 } k; struct { enum { B0 = 7 } m; } in; };
int f(void) {
  struct L { enum { C0 = 11 } c; struct M { int x; } m; } l = { C0, { 5 } };
  struct M n = { l.m.x };
  return l.c + C0 + n.x - 5;
}
int g(void) {
  struct L { struct M { int y; } m; } l = { { 5 } };
  struct M n = l.m;
  return n.y;
}
int main(void) {
  struct S s = { A1, { B0 } };
  printf("%d %d %d %d %d\n", s.k, s.in.m, A0, f(), g());
  return 0;
}
