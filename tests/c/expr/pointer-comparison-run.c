// C17 6.5.8, 6.5.9: the converted operands of a pointer comparison clang
// accepts in C with a warning (distinct pointer types: a bit cast to the left
// operand's type; a pointer against an integer: the integer converted to the
// pointer type), checked at run time. WP-1285.
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 1 1 0 1 1 1 1
//@ diag: 23:39 warning "comparison of distinct pointer types ('int *' and 'long *')"
//@ diag: 23:49 warning "comparison of distinct pointer types ('int *' and 'unsigned int *')"
//@ diag: 24:29 warning "comparison between pointer and integer ('int *' and 'long')"
//@ diag: 24:38 warning "comparison of distinct pointer types ('unsigned int *' and 'int *')"
#include <stdio.h>
typedef int (*kfn)(int);
static int id(int x) { return x; }
int main(void) {
  static int a[4];
  int *ip = &a[1];
  long *lp = (long *)(void *)&a[1];
  unsigned *up = (unsigned *)&a[2];
  kfn k = id;
  long n = (long)ip;
  printf("%d %d %d %d %d %d %d\n", ip == lp, ip < up, k == (void *)0,
         (void *)0 != k, ip == n, up > ip, lp != (long *)up);
  return 0;
}
