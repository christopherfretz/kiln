// GNU __builtin_types_compatible_p in ordinary expressions: an int 0 or 1,
// with __typeof__ operands (the type-generic macro idiom).
//@ expect: run
//@ stdout: 2 1 4
#include <stdio.h>
#define same(a, b) __builtin_types_compatible_p(__typeof__(a), __typeof__(b))
int main(void) {
  int i = 0;
  unsigned u = 0;
  const int ci = 0;
  long l = __builtin_types_compatible_p(int, unsigned) + 2 * same(i, ci) + 4 * same(i, u);
  printf("%ld %d %zu\n", l, __builtin_types_compatible_p(char, char), sizeof same(i, l));
  return 0;
}
