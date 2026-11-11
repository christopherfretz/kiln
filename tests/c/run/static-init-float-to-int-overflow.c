// C17 6.6p7, 6.7.9p4 with C17 6.3.1.4p1: an out-of-range floating to
// integer conversion is undefined, but an arithmetic constant expression
// in a static initializer still folds. clang evaluates on past the
// undefined behaviour with `APFloat`'s saturated value (NaN gives 0,
// clang `HandleFloatToIntCast` -> `HandleOverflow`), and GCC accepts it.
// Before WP-1311 kiln rejected the initializer ("initializer element is
// not a compile-time constant"). C has no dynamic initialization, so
// codegen folds the same value; a static local gets no guard variable.
// (C23 is not pinned: clang 19/22 refuse some of these there, "cannot
// compile this static initializer yet".)
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 2147483647 -2147483648 255 0 -128 0 2147483647
#include <stdio.h>

int g = (int)1e10;
int m = (int)-1e10;
unsigned char h = (unsigned char)300.0;
unsigned r = (unsigned)-1.0;
signed char t = (signed char)-1e3;
int n = (int)__builtin_nan("");

static int local(void) {
  static int v = (int)1e10;
  return v;
}

int main(void) {
  printf("%d %d %d %u %d %d %d\n", g, m, h, r, t, n, local());
  return 0;
}
