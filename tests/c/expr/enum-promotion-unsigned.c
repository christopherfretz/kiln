// C17 6.7.2.2p4, 6.3.1.1p2: an enumeration with no negative enumerator
// is compatible with `unsigned int` (clang's choice), and an integer
// promotion of that type leaves it `unsigned int`; C has no
// [conv.prom]/3 rule that promotes it to `int` (clang
// `computeBestEnumTypes`: `|| !LangOpts.CPlusPlus`). So `?:` with an
// `int` operand and arithmetic on such an enumeration are unsigned.
// Enumerators themselves have type `int` (6.4.4.3p2).
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: unsigned unsigned unsigned int int 1 1 0
#include <stdio.h>

#define T(x) _Generic((x), int: "int", unsigned: "unsigned", default: "other")

enum E { A, B };
enum F { M = -1, N };

int main(int argc, char **argv) {
  (void)argv;
  enum E e = A;
  enum F f = M;
  int i = 0;
  printf("%s %s %s %s %s ", T(+e), T(argc ? e : i), T(e + 0), T(+f), T(A));
  // Wrong code before WP-1311: the `int` promotion made these 0.
  printf("%d %d %d\n", (argc > 5 ? e : -1) > 0, e - 1 > 0, (argc > 5 ? f : -1) > 0);
  return 0;
}
