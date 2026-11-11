// GNU binary conditional `a ?: b` (C23 6.5.16 with the GNU omitted middle
// operand) whose condition is a logical `||`/`&&`: the condition's value is
// the result when it is nonzero. gcc.c-torture compile/pr54103-1.c,
// pr42716.c, pr44119.c, pr89655.c, execute/pr68249.c (20 cases).
//@ std: c17
//@ expect: run
//@ stdout: 7 1 7
//@ xfail(kiln): WP-1282 follow-up, internal compiler error "phi operand %N is defined" (codegen of ?: with a logical condition; C++ too)
#include <stdio.h>
static int f(int a, int b) { return (a || b) ?: 7; }
static int g(int a, int b) { return (a && b) ?: 7; }
int main(void) {
  printf("%d %d %d\n", f(0, 0), f(0, 3), g(1, 0));
  return 0;
}
