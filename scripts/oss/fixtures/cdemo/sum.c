#include "sum.h"

/* `restrict` is a top-level qualifier: the definition still matches the
   unqualified prototype the C++ program sees. */

int sum_points(const struct point *restrict p, int n) {
  int s = 0;
  for (int i = 0; i < n; i++)
    s += p[i].x * p[i].y;
  return s;
}
