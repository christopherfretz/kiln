// [dcl.asm] (GNU extension): `asm goto` lists the labels its template may
// transfer control to; an empty template falls through, a branch to `%l0`
// (or `%l[name]`) reaches that label, and outputs are stored on every edge.
//@ expect: run
//@ stdout: 0 1 7 8 9
#include <stdio.h>
#if defined(__aarch64__)
#define JMP "b "
#else
#define JMP "jmp "
#endif
int f() {
  asm goto("" : : : : taken);
  return 0;
taken:
  return 1;
}
int g() {
  asm goto(JMP "%l0" : : : : taken);
  return 0;
taken:
  return 1;
}
int h(int x) {
  int y = x;
  asm goto(JMP "%l[out]" : "+r"(y) : : : skip, out);
  return y;
skip:
  return -1;
out:
  return y + 1;
}
int k(int x) {
  int y;
  asm goto("" : "=r"(y) : "0"(x) : : out);
  return y + 1;
out:
  return -1;
}
int main() { printf("%d %d %d %d %d\n", f(), g(), h(6), k(7), h(8)); }
