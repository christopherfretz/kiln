// [expr.cond] GNU binary conditional `a ?: b` whose condition is a logical
// `||`/`&&` (a prvalue bool): the result is the condition's value when it
// is true. Found by WP-1282 in gcc.c-torture (compile/pr54103-1.c).
//@ expect: run
//@ stdout: 7 1 7
//@ xfail(kiln): WP-1282 follow-up, internal compiler error "phi operand %N is defined" (codegen of ?: with a logical condition)
#include <cstdio>
static int f(int a, int b) { return (a || b) ?: 7; }
static int g(int a, int b) { return (a && b) ?: 7; }
int main() {
  std::printf("%d %d %d\n", f(0, 0), f(0, 3), g(1, 0));
  return 0;
}
