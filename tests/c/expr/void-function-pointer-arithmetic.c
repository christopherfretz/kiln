// GNU C (clang and GCC, on by default): arithmetic on a pointer to `void` or to
// a function treats the element size as 1 (C17 6.5.6p2 requires a complete
// object type; -Wpointer-arith is off by default), `sizeof` and `_Alignof` of
// `void` or a function type are 1 (C17 6.5.3.4p1 constraint), and `void *` may
// be subscripted. WP-1285 (gcc.c-torture compile/20021108-1.c).
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 3 4 -1 1 1 1 1 2 1 1 7 6
#include <stdio.h>
static void g(void) {}
int main(void) {
  static char buf[8] = "0123456";
  void *p = buf;
  void *q = p + 3;
  q++;
  void (*f)(void) = g;
  void (*f2)(void) = f + 2;
  f2 -= 1;
  long d = (char *)f2 - (char *)f;
  void *e = &p[1];
  --q;
  void *r = 7 + p;
  r = r - 0;
  printf("%d %d %ld %d %d %d %d %d %ld %d %c %c\n",
         (int)((char *)q - buf), (int)(q - p + 1), p - e,
         (int)sizeof(void), (int)sizeof(g), (int)sizeof g, (int)_Alignof(void),
         (int)(sizeof(void) + sizeof(void (void))), d, (int)(e - p),
         ((char *)p)[7 - 0] == 0 ? '7' : '?', *(char *)(r - 1));
  return 0;
}
