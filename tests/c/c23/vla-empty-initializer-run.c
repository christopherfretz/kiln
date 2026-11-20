// C23 6.7.11/4 (WP-1739): the empty initializer `= {}` may initialize a
// variable length array, and does so to zero (6.7.11/11): one of a
// scalar, of a structure, multidimensional with a variable outer or
// inner bound, a one-byte element. The stack under the arrays is dirtied
// first, so a missing zeroing shows. clang accepts it in every C mode
// (before C23 as an extension, vla-empty-initializer-c17.c).
//@ expect: run
//@ std: c23
//@ stdout: 0 0 0 0 0 0
int printf(const char *, ...);

struct P { int x; double y; };

static void dirty(void) {
  volatile unsigned char junk[4096];
  for (int i = 0; i < 4096; ++i) junk[i] = (unsigned char)(i | 1);
}

static long sum_int(int n) {
  int a[n] = {};
  long s = 0;
  for (int i = 0; i < n; ++i) s += a[i];
  return s;
}

static double sum_p(int n) {
  struct P p[n][2] = {};
  double s = 0;
  for (int i = 0; i < n; ++i)
    for (int j = 0; j < 2; ++j) s += p[i][j].x + p[i][j].y;
  return s;
}

static int sum_inner(int n, int m) {
  short a[3][m] = {};
  int s = 0;
  for (int i = 0; i < 3; ++i)
    for (int j = 0; j < m; ++j) s += a[i][j];
  return s + (int)(sizeof a != 3 * m * sizeof(short));
}

static int sum_char(int n) {
  char c[n] = {};
  int s = 0;
  for (int i = 0; i < n; ++i) s += c[i];
  return s;
}

int main(void) {
  dirty();
  long a = sum_int(100);
  dirty();
  double b = sum_p(50);
  dirty();
  int c = sum_inner(2, 70);
  dirty();
  int d = sum_char(300);
  dirty();
  int n = 7;
  int e[n] = {};
  dirty();
  int f[n][n] = {};
  int s = 0;
  for (int i = 0; i < n; ++i) s += e[i] + f[i][n - 1];
  printf("%ld %d %d %d %d %d\n", a, (int)b, c, d, s, (int)(sizeof f / sizeof f[0]) - 7);
  return 0;
}
