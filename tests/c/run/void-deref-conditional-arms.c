// C17 6.5.15p5 (both operands of `?:` void) and 6.5.3.2p4 (`*p` of a
// pointer to qualified `void` is a void expression, a GNU extension):
// a conditional, comma, cast to `void` or statement expression whose
// operands are such void expressions runs their side effects only; no
// operand is loaded, not even a `volatile void` one (clang emits no load).
// WP-1347 (gcc.c-torture compile/20000701-1.c).
//@ std: c17
//@ args: -w
//@ expect: run
//@ stdout: 7 13
#include <stdio.h>
static int calls;
static void *next(void *p) { ++calls; return p; }
static const void *nextc(const void *p) { ++calls; return p; }
static volatile void *nextv(volatile void *p) { ++calls; return p; }
int main(void) {
  int x = 7;
  void *pv = &x;
  const void *pcv = &x;
  volatile void *pvv = &x;
  int i = 1, z = 0;
  i ? *next(pv) : *next(pv);
  z ? *next(pv) : *next(pv);
  1 ? *next(pv) : *pv;
  i ? *nextc(pcv) : *nextc(pcv);
  i ? *nextv(pvv) : *nextv(pvv);
  i ? *pvv : *pv;
  i ? (z ? *next(pv) : *next(pv)) : *pv;
  z ? (void)0 : *next(pv);
  i ? *next(pv) : (void)0;
  *next(pv), *next(pv);
  (void)*nextv(pvv);
  *pvv;
  ({ *next(pv); });
  (void)({ i ? *next(pv) : *pv; });
  (void)(i ? *pv : *pv);
  printf("%d %d\n", *(int *)pv, calls);
  return 0;
}
