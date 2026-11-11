// GNU extension accepted in GNU C and by clang in C mode: arithmetic on
// `void *` treats the pointee as size 1 (and sizeof(void) is 1). Used in
// computed-goto label offsets (gcc.c-torture compile/labels-1.c) and by
// real C code doing byte arithmetic on void pointers.
//@ std: c17
//@ expect: run
//@ stdout: 3 4
//@ strict-diags
#include <stdio.h>
int main(void) {
  static char buf[8];
  void *p = buf;
  void *q = p + 3;
  q++;
  printf("%d %d\n", (int)((char *)q - buf - 1), (int)(q - p));
  return 0;
}
