// C23 5.1.2.3.2 (main), 6.10.10.2 (__STDC_VERSION__): hello world at
// -std=c17, which defines __STDC_VERSION__ as 201710L.
//@ std: c17
//@ expect: run
//@ stdout: hello, c17 201710
#include <stdio.h>
#include <string.h>
#include <math.h>
int main(void) {
  const char *s = "hello, c17";
  printf("%s %ld\n", s, (long)__STDC_VERSION__ + (long)(strlen(s) - strlen(s)) - (long)!(HUGE_VAL > 1.0));
  return 0;
}
