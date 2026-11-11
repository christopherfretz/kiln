// C23 6.6p9-p10 (GNU C, as clang): an address converted to an integer as
// wide as a pointer stays an address constant under integer additions and
// subtractions of constants (`(unsigned long)&x - 0x10 - 1`, clang's
// `ptrtoint (gep i8 @x, -17)`), and a block-scope static so initialized is
// static data.
//@ expect: run
//@ stdout: 1 1 1 1
#include <stdint.h>
#include <stdio.h>
char text[64];
static unsigned long x = (unsigned long)&text - 0x10L - 1;
static uintptr_t y = (uintptr_t)&text[8] + 4;
static long z = 3 + (long)&text;
int main(void) {
  static uintptr_t w = (uintptr_t)text - 1;
  printf("%d %d %d %d\n", x == (unsigned long)&text - 17, y == (uintptr_t)&text[12],
         z == (long)&text[3], w + 1 == (uintptr_t)text);
  return 0;
}
