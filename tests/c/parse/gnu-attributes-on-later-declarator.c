// C17 6.7/1, 6.7.2.1/1 (GNU extension): a later
// init-declarator or member-declarator of a group may begin with GNU
// attributes, which apply to that declarator alone (clang's
// `ParseDeclGroup` and `ParseStructDeclaration`; gcc.c-torture
// compile/20010714-1.c).
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 1 16 16 4
#include <stddef.h>
#include <stdio.h>

void d0(void), __attribute__((noreturn)) d1(void), d2(void);
int a, __attribute__((aligned(16))) b, c;

struct S {
  char x, __attribute__((aligned(16))) y;
  int z;
};

void d1(void) { for (;;) ; }

int main() {
  printf("%d %d %d %d\n", (int)__alignof__(a) <= 8,
              (int)__alignof__(b), (int)offsetof(struct S, y),
              (int)__alignof__(c));
}
