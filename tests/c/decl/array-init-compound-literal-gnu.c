// C23 6.7.10p16 (initialization): an array is initialized by a brace list
// or a string literal; GNU C also accepts a compound literal (or a
// parenthesized string literal) of compatible array type, copying its
// elements (clang SK_ArrayInit, -Wgnu-compound-literal-initializer under
// -pedantic). gcc.c-torture: compile/pr45919.c, execute/pr98407.c.
//@ std: c17
//@ expect: run
//@ stdout: 4 3371 7 9 3 1 x yz
#include <stdio.h>
static const unsigned short g[] = (const unsigned short[]){0x0D2B, 7};
static int f(void) {
  int b[] = (int[]){1, 2, 3};
  return b[2] + (int)sizeof b / (int)sizeof b[0] + 3;
}
char a[] = ("x");
const char p[] = (("yz"));
int main(void) {
  int n[2] = (int[2]){3, 1};
  printf("%d %d %d %d %d %d %s %s\n", (int)sizeof g, g[0], g[1], f(), n[0], n[1], a, p);
  return 0;
}
