// C23 6.7.7.3p6, 6.2.7p1, 6.5.7p3, 6.5.17.2p1, 6.5.2.5, 6.5.4p3,
// 6.5.1.1p2: two array types with compatible element types are
// compatible when either size is not an integer constant expression (the
// behavior is undefined only if the sizes differ at run time), so a
// pointer to a variable length array converts to and from a pointer to
// another one of the same element type, and to one of constant size, by
// initialization, assignment and cast, and two such pointers can be
// subtracted and compared. `sizeof`, pointer arithmetic and `_Generic`
// (whose controlling type may be variably modified) keep working. WP-1386.
//@ std: c23
//@ expect: run
//@ stdout: 6 4 12 1 1 1 4 20
#include <stdio.h>
static int sum(int n, int (*a)[n]) { return a[0][0] + a[0][n - 1]; }
int main(void) {
  int m = 3, k = 3;
  int buf[2][3] = {{1, 2, 3}, {4, 5, 6}};
  void *x = buf;
  int (*p)[m] = (int (*)[m])x;
  int (*q)[k] = p;
  int (*r)[3] = q;
  q = p + 1;
  typedef int R[m];
  R *t = (R *)buf;
  int (*u)[k] = t;
  printf("%d %d %zu %d %d %d %d %zu\n", p[1][2], r[1][0], sizeof *p, (int)(q - p),
         (int)(p == q - 1), _Generic(p, int (*)[3]: 1, default: 0), sum(m, u),
         sizeof(int[m]) + sizeof(int[k - 1]));
  return 0;
}
