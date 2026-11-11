// C89 2.1.2.2 (C23 5.1.2.3.2): a hosted program starts at main. C89
// defines __STDC__ as 1 and has no __STDC_VERSION__ (C95 added it).
//@ std: c89
//@ skip(gcc): GCC has no line comments in C89, and this header is one
//@ expect: run
//@ stdout: hello, c89 1 1
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
int main(void) {
  char buf[16];
  int n;
#ifdef __STDC_VERSION__
  n = 0;
#else
  n = 1;
#endif
  strcpy(buf, "hello, c89");
  printf("%s %d %d\n", buf, __STDC__, n);
  return EXIT_SUCCESS;
}
