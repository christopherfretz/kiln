// [expr.cond] GNU binary conditional `a ?: b` whose condition is a logical
// `||`/`&&` (a prvalue bool): the result is the condition's value when it
// is true. Found by WP-1282 in gcc.c-torture (compile/pr54103-1.c); the
// common operand's `zext` must survive the int-to-bool peephole (clang
// `protectFromPeepholes`), WP-1288. The -O2 build is
// gnu-binary-conditional-logical-O2-run.cpp.
//@ expect: run
//@ stdout: 7 1 7 1 9 8 0 1 2
#include <cstdio>
static int f(int a, int b) { return (a || b) ?: 7; }
static int g(int a, int b) { return (a && b) ?: 7; }
static int nested(int a, int b, int c) { return ((a && b) || c) ?: ((a || c) ?: 9); }
static int cmp(int a, int b) { return (a < b) ?: 8; }
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
int main() {
  std::printf("%d %d %d %d %d %d %d %d %d\n", f(0, 0), f(0, 3), g(1, 0), nested(1, 1, 0),
              nested(0, 0, 0), cmp(3, 2), cond(0, 0, 0), cond(0, 0, 4), loop(5));
  return 0;
}
