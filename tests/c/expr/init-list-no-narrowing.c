// C23 6.7.11p11, p13 (initialization): a brace initializer converts each
// value as by assignment (6.5.17.2); C has no narrowing rule, so a
// non-constant int initializes a char member. gcc.c-torture
// execute/20000113-1.c and many others.
//@ std: c17
//@ expect: run
//@ stdout: 44 7
//@ xfail(kiln): WP-1282 follow-up, C++ narrowing check (-Wc++11-narrowing) applied to a C brace initializer
#include <stdio.h>
struct s { unsigned char c; short h; };
int main(void) {
  int i = 300;
  long l = 7;
  struct s v = { i, l };
  printf("%d %d\n", v.c, v.h);
  return 0;
}
