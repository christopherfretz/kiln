// C23 6.5.10p2 (equality operators): a pointer to function may be compared
// with a null pointer constant, including `(void *)0` (6.3.2.3p3). lua's
// ldo.c compares an lua_KFunction with NULL.
//@ std: c17
//@ expect: run
//@ stdout: 1 0
#include <stdio.h>
typedef int (*kfn)(int);
static int id(int x) { return x; }
static int is_null(kfn k) { return k == ((void *)0); }
static int not_null(kfn k) { return (void *)0 != k; }
int main(void) {
  printf("%d %d\n", is_null(0), not_null(0) + !not_null(id));
  return 0;
}
