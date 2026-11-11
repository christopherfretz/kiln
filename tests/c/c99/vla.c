// C17 6.7.6.2p4-p5, 6.5.3.4p2: variable length arrays; sizeof of a VLA
// is evaluated at run time; VLA parameters, [*] in a prototype, and
// static/qualifiers inside the brackets of an array parameter.
//@ expect: run
//@ xfail(kiln): WP-1276 follow-up, variable length arrays are not implemented in C (sema "not supported yet"; [*] and static in array parameter brackets are not parsed in C)
//@ stdout: 40 12 5 15
int printf(const char *, ...);
int total(int n, int m, int a[n][m]);
int total(int n, int m, int a[*][*]);
int total(int n, int m, int a[static n][m]) {
  int s = 0;
  for (int i = 0; i < n; i++)
    for (int j = 0; j < m; j++) s += a[i][j];
  return s;
}
int first(int a[const static 1]) { return a[0]; }
int main(void) {
  int n = 10;
  int v[n];
  int r = 3, c = 2;
  int m[r][c];
  for (int i = 0; i < r; i++)
    for (int j = 0; j < c; j++) m[i][j] = i + j + 1;
  int t = total(r, c, m);
  printf("%zu %zu %d %d\n", sizeof v, sizeof m / sizeof m[0][0] * 2, first(m[2]) + 2, t);
  return 0;
}
