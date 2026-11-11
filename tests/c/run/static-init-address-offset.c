// C23 6.6p9 (GNU C, as clang): an address constant is the address of an
// object or function plus or minus an integer constant expression, which
// clang folds even past the object (`data + 128`, `(char *)f + 1`, an
// integer cast to a pointer that does not fit 32 bits such as `(void *)-1`),
// a member array without a bound decayed (`&c.sc[0]`), and a file-scope
// compound literal's address cast to another pointer type.
//@ expect: run
//@ stdout: 1 1 1 1 1 1
#include <stdint.h>
#include <stdio.h>
static const unsigned short data[] = { 1 };
const unsigned short *cb = data + 128;
static int f(void) { return 0; }
char *fp = (char *)f + 1;
void *m1 = (void *)-1;
void *m2 = (void *)0x100000000L;
typedef struct { int disable; char *sc[]; } config_t;
config_t config = { .sc = { (void *)0 } };
struct { void *lng; } arglist[] = { { &config.sc[0] } };
typedef struct { long type; char *value; } T3;
struct { char *k; char *v; } a[] = { { "", (char *)&((T3){ 1, (char *)1 }) } };
int main(void) {
  printf("%d %d %d %d %d %d\n", cb == data + 128, fp == (char *)f + 1,
         (uintptr_t)m1 == UINTPTR_MAX, (uintptr_t)m2 == 0x100000000UL,
         arglist[0].lng == (void *)config.sc, ((T3 *)a[0].v)->type == 1);
  return 0;
}
