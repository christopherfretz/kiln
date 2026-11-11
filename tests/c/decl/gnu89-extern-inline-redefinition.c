// GNU89 inline semantics (-std=gnu89, or the gnu_inline attribute): an
// `extern inline` definition provides only an inline body, and a later
// external or static definition replaces it (clang `canRedefineFunction`);
// calls through the address take the out-of-line definition. In C99
// semantics the redefinition is an error (C23 6.7.5p7, 6.9p5).
//@ args: -std=gnu89
//@ expect: run
//@ stdout: 2 2 4 4 6
#include <stdio.h>
extern inline int f(void) { return 1; }
int f(void) { return 2; }
extern inline int g(void) { return 3; }
static inline int g(void) { return 4; }
extern inline __attribute__((gnu_inline)) int h(void) { return 5; }
int h(void) { return 6; }
int (*pf)(void) = f;
int (*pg)(void) = g;
int main(void) {
  printf("%d %d %d %d %d\n", f(), pf(), g(), pg(), h());
  return 0;
}
