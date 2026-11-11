// C23 6.3.2.3p6 (pointers): any pointer type may be converted to an integer
// type, and the result is implementation-defined. A cast to an integer narrower
// than a pointer is valid C; C++ rejects it ([expr.reinterpret.cast]p4). clang
// warns -Wpointer-to-int-cast (-Wvoid-pointer-to-int-cast from `void *`, WP-1285;
// both CI targets are LP64).
// gcc.c-torture has 37 such tests (e.g. compile/20000105-2.c).
//@ std: c17
//@ expect: run
//@ stdout: 0 0 1
//@ strict-diags
//@ diag: 18:11 warning "cast to smaller integer type 'int' from 'void *'"
//@ diag: 19:21 warning "cast to smaller integer type 'unsigned char' from 'void *'"
//@ diag: 21:14 warning "cast to smaller integer type 'int' from 'char *'"
//@ diag: 21:29 warning "cast to smaller integer type 'int' from 'char *'"
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
