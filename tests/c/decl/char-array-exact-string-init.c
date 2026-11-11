// C23 6.7.10p15 (initialization): a character array may be initialized by a
// string literal whose characters, not counting the terminating null, fill
// the array exactly; the null is then dropped. C++ rejects it ([dcl.init.string]p2).
// gcc.c-torture: execute/strlen-6.c, execute/20010924-1.c.
//@ std: c17
//@ expect: run
//@ stdout: 3 abc
//@ xfail(kiln): WP-1282 follow-up, "initializer-string for char array is too long" when only the null does not fit
#include <stdio.h>
static char s[3] = "abc";
int main(void) {
  printf("%d %.3s\n", (int)sizeof s, s);
  return 0;
}
