// C17 6.5.3.2p4: `*p` of a pointer to (qualified) `void` is an expression of
// type `void` (C DR 106), usable where a void expression is: discarded, an
// operand of `?:` or `,`, or cast to `void`. clang accepts it with the
// on-by-default `-Wvoid-ptr-dereference` extension warning; C++ rejects it
// ([expr.unary.op]/1). WP-1285 (gcc.c-torture compile/20000701-1.c).
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 7 3
//@ diag: 23:9 warning "ISO C does not allow indirection on operand of type 'void *'"
//@ diag: 24:9 warning "ISO C does not allow indirection on operand of type 'void *'"
//@ diag: 25:21 warning "ISO C does not allow indirection on operand of type 'const void *'"
//@ diag: 26:10 warning "ISO C does not allow indirection on operand of type 'void *'"
//@ diag: 26:21 warning "ISO C does not allow indirection on operand of type 'void *'"
//@ diag: 26:10 warning "left operand of comma operator has no effect"
#include <stdio.h>
static int calls;
static void *next(void *p) { ++calls; return p; }
int main(void) {
  int x = 7;
  void *pv = &x;
  const void *pcv = &x;
  (void)*pv;
  (void)*next(pv);
  calls ? (void)0 : *pcv;
  (void)(*next(pv), *next(pv));
  printf("%d %d\n", *(int *)pv, calls);
  return 0;
}
