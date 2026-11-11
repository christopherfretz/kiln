// The program behind vla-runtime.c (-O0) and vla-runtime-o2.c (-O2):
// C17 6.7.6.2p4-p5, 6.5.3.4p2, 6.7.8p3, 6.5.6p8-p9, 6.9.1p10.
int printf(const char *, ...);

// 6.9.1p10: the bounds of a variably modified parameter are evaluated on
// entry; `sizeof a[0]` is n * sizeof(int) at run time.
static long sum2(int n, int m, int a[n][m]) {
  long s = 0;
  for (int i = 0; i < n; i++)
    for (int j = 0; j < m; j++) s += a[i][j];
  return s * 1000 + (long)(sizeof a[0] / sizeof a[0][0]);
}

// A pointer to a variable length array: arithmetic steps over m ints.
static int walk(int n, int m, int (*p)[m]) {
  int (*end)[m] = p + n;
  int s = 0, rows = 0;
  for (int (*q)[m] = p; q != end; q++, rows++) s += (*q)[m - 1];
  return s * 10 + rows * 100 + (int)((char *)end - (char *)p);
}

static int counter;
static int next(void) { return ++counter; }

int main(void) {
  int n = 4;
  int v[n];
  // 6.5.3.4p2: sizeof of a VLA is evaluated at run time.
  printf("%zu", sizeof v);
  // 6.7.8p3: a VLA typedef's size is fixed where the typedef is reached.
  typedef int Row[n];
  n = 7;
  Row r;
  printf(" %zu %zu", sizeof r, sizeof(Row));
  // The operand of sizeof is evaluated when it has VLA type.
  int k = 2;
  unsigned long z = sizeof(int[k++]);
  printf(" %lu %d", z, k);
  // A two-dimensional VLA, passed as `int a[n][m]` and through a pointer
  // to a VLA.
  int rows = 3, cols = 5;
  int m[rows][cols];
  for (int i = 0; i < rows; i++)
    for (int j = 0; j < cols; j++) m[i][j] = i * 10 + j;
  printf(" %ld %d", sum2(rows, cols, m), walk(rows, cols, m));
  // A cast to a pointer to a VLA evaluates its bound.
  int *flat = &m[0][0];
  int w = (*((int (*)[next() + 4])flat + 1))[2];
  printf(" %d %d", w, counter);
  // The stack is restored at each iteration's end: 20000 iterations of a
  // 64 KiB array would overflow the stack otherwise.
  long acc = 0;
  for (int it = 0; it < 20000; it++) {
    int big[16384 + (it & 1)];
    big[it % 100] = it;
    acc += big[it % 100];
  }
  printf(" %ld", acc);
  // Jumping out of a VLA's scope (allowed) restores the stack too.
  for (int it = 0; it < 20000; it++) {
    int big[16384 + (it & 1)];
    big[0] = it;
    if (big[0] >= 0) goto next_it;
    return 1;
  next_it:;
  }
  printf(" ok\n");
  return 0;
}
