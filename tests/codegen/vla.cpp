// Variable length arrays (GNU extension, -Wvla-cxx-extension): bounds
// evaluated once, llvm.stacksave/stackrestore per scope, dynamic allocas,
// scaled subscripts, runtime sizeof, typedef bounds, EH cleanups, and a
// static local whose bound folds to a constant array.
int f(int n) {
  int a[n];
  a[0] = 1;
  for (int i = 0; i < n; ++i) a[i] = i;
  return a[n - 1] + sizeof(a);
}
void g(int n) {
  {
    char b[n * 2];
    b[0] = 0;
  }
  const int c = 4;
  static int s[c];
  s[1] = 2;
}
struct P { int x; char c; };
unsigned long f2(short n, long j) {
  int m[n][4];
  m[1][2] = 3;
  int k[3][n];
  k[j][1] = 5;
  P ps[n];
  ps[j].c = 'a';
  double d[n + 1];
  d[j] = 1.5;
  unsigned long t = sizeof(m) + sizeof(k) + sizeof(ps) + sizeof d;
  return t + sizeof(char[n]) + sizeof(int[n]);
}
int loop(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) {
    int a[i + 1];
    a[i] = i;
    if (i == 5) break;
    if (i == 3) continue;
    s += a[i];
  }
  while (n--) {
    char b[n];
    if (n == 2) return s;
  }
  return s;
}
int early(int n) {
  int a[n];
  a[0] = n;
  if (n > 3) return a[0];
  int b[n];
  b[0] = 1;
  int *p = a;
  int *q = &b[0];
  return *p + *q + 0[a];
}
void tdef(int n) {
  typedef int T[n];
  T x;
  x[0] = 1;
  using U = char[n * 2];
  U y;
  y[1] = 2;
}
int pv(int n, int i) {
  int a[n];
  auto p = &a;
  (*p)[i] = 4;
  int m[5][n];
  auto q = m;
  q[i][1] = 2;
  auto &r = a;
  r[1] = 3;
  typedef int T[n];
  n++;
  T x;
  x[0] = n;
  return a[i] + sizeof(x) + sizeof(*q) + sizeof r;
}
int g3(int n, int i, int j) {
  int g[n][n + 1];
  g[i][j] = 1;
  g[1][2] = 3;
  char h[2][n][3][n];
  h[1][i][2][j] = 4;
  return g[j][i] + sizeof(g) + sizeof(g[1]) + sizeof(h[i]) + sizeof(h[0][1]);
}
const _Atomic(int) ca = 4;
int g4(int n) {
  static int s[ca];
  int u[ca];
  u[0] = 1;
  s[1] = 2;
  return s[1] + sizeof(u);
}
void may_throw(int *);
struct D { ~D(); };
void eh(int n) {
  D d;
  int a[n];
  may_throw(a);
  {
    D e;
    int b[n];
    may_throw(b);
  }
  switch (n) {
  case 1: {
    int c[n];
    may_throw(c);
    break;
  }
  default:
    break;
  }
}
