// C23 6.7.11p4 and GNU `__builtin_constant_p`: an initializer of an object
// with static storage duration is evaluated in a constant context, where
// `__builtin_constant_p` of an expression that is not a constant is 0 (clang
// and GCC fold it so), not a non-constant initializer.
//@ expect: run
//@ stdout: 0 1 0
#include <stdio.h>
int g = 5;
int a = __builtin_constant_p(g) ? 1 : 0;
int b = __builtin_constant_p(3 * 4);
int main(int argc, char **argv) {
  (void)argv;
  static int c[] = { __builtin_constant_p(argc) ? 1 : 0 };
  printf("%d %d %d\n", a, b, c[0]);
  return 0;
}
