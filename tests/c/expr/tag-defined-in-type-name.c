// C17 6.7.2.3p6-7, 6.7.7p2, 6.2.1p4: a struct, union or enum may be defined
// in any type name -- a cast, `sizeof`, `_Alignof`, `__builtin_offsetof` or a
// function's return type -- and the tag is then declared in the enclosing
// scope (C++ forbids this, [dcl.type.general]/3, [expr.cast]/3). A
// forward-referenced `enum` (`enum F;` before its definition) is a GNU
// extension, warned only under -Wpedantic. WP-1285 (gcc.c-torture
// compile/990928-1.c, 20000211-1.c).
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 8 1 4 1 1 2 4 8 1
#include <stdio.h>
int a = sizeof(struct A { int x; int x2; });
int b = (struct B { int y; } *)0 == 0;
struct R { int z; } f(void) { struct R r = {1}; return r; }
int c = _Alignof(union U { int q; char w; });
int g(void *p) {
  ((struct C { int k; } *)p)->k = sizeof(enum E2 { E2a, E2b });
  struct A aa = {1, 2};
  struct B bb = {3};
  union U uu = {4};
  enum E2 ee = E2b;
  return aa.x + aa.x2 + bb.y + uu.q + (int)ee - 10;
}
enum F;
enum F *fp;
int h(enum F x);
enum F { F1, F2 };
int h(enum F x) { return x == F2; }
int s2 = sizeof(struct { int a, b; });
int k(void) { return __builtin_offsetof(struct O { int a; int b; }, b); }
int main(void) {
  int store = 0;
  int gv = g(&store);
  printf("%d %d %d %d %d %d %d %d %d\n", a, b, c, f().z, gv, store - 2, k(), s2, h(F2));
  return 0;
}
