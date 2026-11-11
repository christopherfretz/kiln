// GNU binary conditional `a ?: b` (C23 6.5.16 with the GNU omitted middle
// operand) whose condition is a logical `||`/`&&`, a relational or a `!`
// (each an `int` that is a zero-extended `i1`): the condition's value is
// the result when it is nonzero. gcc.c-torture compile/pr54103-1.c,
// pr42716.c, pr44119.c, pr89655.c, execute/pr68249.c (20 cases); fixed by
// WP-1288 (the `zext` of the common operand survives the int-to-bool
// peephole). The -O2 build is gnu-binary-conditional-logical-O2.c.
//@ std: c17
//@ expect: run
//@ stdout: 7 1 7 1 9 8 6 0 1 2
#include <stdio.h>
static int f(int a, int b) { return (a || b) ?: 7; }
static int g(int a, int b) { return (a && b) ?: 7; }
static int nested(int a, int b, int c) { return ((a && b) || c) ?: ((a || c) ?: 9); }
static int cmp(int a, int b) { return (a < b) ?: 8; }
static int not_(int a) { return (!a) ?: 6; }
static int cond(int a, int b, int c) {
  if ((a || b) ?: c)
    return 1;
  return 0;
}
static int loop(int n) {
  int k = 0;
  while ((n > 0 && k < 2) ?: 0) {
    ++k;
    --n;
  }
  return k;
}
int main(void) {
  printf("%d %d %d %d %d %d %d %d %d %d\n", f(0, 0), f(0, 3), g(1, 0), nested(1, 1, 0),
         nested(0, 0, 0), cmp(3, 2), not_(5), cond(0, 0, 0), cond(0, 0, 4), loop(5));
  return 0;
}
