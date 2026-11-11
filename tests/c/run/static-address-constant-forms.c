// C23 6.6p9 (address constants), 6.7.11p4: an initializer of an object with
// static storage duration may be an address constant formed through casts:
// a pointer cast to another object pointer type (`(T *)&x`,
// `(char *)&arr[3]`), a function cast to `void *`, an integer cast to a
// pointer, an address plus an offset, and (6.6p10, as clang) an address
// cast to an integer as wide as a pointer. Each is static data, so a
// block-scope static is set before program startup: the `goto` past the
// declarations (6.8.7.2p1) still sees every value.
//@ expect: run
//@ stdout: 1 1 1 16 1 1 1 1 1 | 1 1 1 1 1
#include <stdint.h>
#include <stdio.h>
struct S { int a, b; } gs = { 4, 5 };
int g;
char arr[10] = "abcdefghi";
static int one(void) { return 1; }
char *p1 = (char *)&gs;
char *p2 = (char *)&arr[3];
void *p3 = (void *)one;
void *p4 = (void *)16;
int *p5 = (int *)&gs + 1;
intptr_t i1 = (intptr_t)&g;
char *p6 = (char *)&gs.b + 2;
int (*p7)(void) = (int (*)(void))(void *)one;
struct T { intptr_t x; int y; uintptr_t z; } t = { (intptr_t)&g, 2, (uintptr_t)one };
int main(void) {
  printf("%d %d %d %d %d %d %d %d %d | ", p1 == (char *)&gs, *p2 == 'd',
         p3 == (void *)one, (int)(intptr_t)p4, *p5 == 5, i1 == (intptr_t)&g,
         p6 == (char *)&gs + sizeof(int) + 2, p7() == 1,
         t.x == (intptr_t)&g && t.y == 2 && t.z == (uintptr_t)one);
  goto skip;
  static unsigned char *const sp[] = { (unsigned char *)" " };
  static char *q = (char *)&gs.b;
  static void *n = (void *)(intptr_t)5;
  static uintptr_t u = (uintptr_t)&arr[2];
  static const struct { void *d; int (*fn)(void); } fd[] = { { (void *)8, one } };
skip:
  printf("%d %d %d %d %d\n", sp[0][0] == ' ', q == (char *)&gs.b,
         (int)(intptr_t)n == 5, u == (uintptr_t)&arr[2],
         fd[0].d == (void *)8 && fd[0].fn() == 1);
  return 0;
}
