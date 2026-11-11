// C17 6.7/1, 6.7.5: an alignment-specifier is a declaration specifier, so
// it may follow a type specifier in any order (`unsigned short
// _Alignas(16) in[8];`), in a declaration and a member declaration
// (clang `ParseDeclarationSpecifiers`; gcc.c-torture execute/pr68532.c).
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 16 32 32 16
#include <stddef.h>
#include <stdio.h>

unsigned short _Alignas(16) in[8];
static int const _Alignas(32) k = 1;

struct S {
  char c;
  short _Alignas(32) s;
};

int main(void) {
  long _Alignas(16) local = k;
  printf("%d %d %d %d\n", (int)__alignof__(in), (int)__alignof__(k),
         (int)offsetof(struct S, s), (int)__alignof__(local) + (int)local - 1);
  return 0;
}
