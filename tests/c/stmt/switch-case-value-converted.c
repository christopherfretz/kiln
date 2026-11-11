// C23 6.8.5.3p5 (the switch statement): each case constant is converted to
// the promoted type of the controlling expression; no narrowing rule applies
// (C++ requires a converted constant expression, [stmt.switch]p2). sqlite's
// shell.c has `case 0xca093fa0:` in a switch on an int.
//@ std: c17
//@ expect: run
//@ stdout: 1 2
//@ xfail(kiln): WP-1282 follow-up, "case value evaluates to N, which cannot be narrowed" applied to C case labels
#include <stdio.h>
static int pick(int op) {
  switch (op) {
  case 7:
    return 2;
  case 0xca093fa0:
    return 1;
  }
  return 0;
}
int main(void) {
  printf("%d %d\n", pick((int)0xca093fa0u), pick(7));
  return 0;
}
