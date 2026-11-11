// C23 6.3.2.3p6 (pointers): any pointer type may be converted to an integer
// type, and the result is implementation-defined. A cast to an integer narrower
// than a pointer is valid C; C++ rejects it ([expr.reinterpret.cast]p4).
// gcc.c-torture has 37 such tests (e.g. compile/20000105-2.c).
//@ std: c17
//@ expect: run
//@ stdout: 0 0 1
//@ xfail(kiln): WP-1282 follow-up, "cast from pointer to smaller type loses information" applied to C casts
#include <stdio.h>
int main(void) {
  void *p = 0;
  int i = (int)p;
  unsigned char c = (unsigned char)p;
  static char buf[2];
  int same = (int)&buf[1] - (int)&buf[0];
  printf("%d %d %d\n", i, c, same);
  return 0;
}
