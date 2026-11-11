// C17 6.2.1p4, 6.7.2.3p6: a struct, union or enum may be defined in a
// parameter type; its scope is the prototype (or, in a definition, the
// function), so clang warns that the declaration "will not be visible outside
// of this function" (-Wvisibility) for a named tag or any enumeration, and not
// for an unnamed struct or union. C++ rejects the definition
// ([dcl.fct]/17). WP-1285 (gcc.c-torture compile/pr46866.c).
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 3 5 1
//@ diag: 14:16 warning "declaration of 'struct P' will not be visible outside of this function"
//@ diag: 19:15 warning "declaration of 'enum Sel' will not be visible outside of this function"
#include <stdio.h>
int sum(struct P { int x, y; } p) {
  struct P q = p;
  return q.x + q.y;
}
int first(union { int i; float f; } u) { return u.i; }
int pick(enum Sel { LEFT, RIGHT } s, int a, int b) { return s == RIGHT ? b : a; }
int main(void) {
  printf("%d %d %d\n", 3, pick(1, 4, 5), 1);
  return 0;
}
