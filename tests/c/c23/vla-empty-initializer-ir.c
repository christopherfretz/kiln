// C23 6.7.11/4 (WP-1739): a variable length array's empty initializer
// is a `memset` to zero of its run-time size, the element count times
// the element size (none for a one-byte element; clang
// `EmitNullInitialization`).
//@ expect: ir
//@ std: c23
struct P { int x; double y; };
void g(void *);
void f(int n, int m) {
  int a[n] = {};
  struct P p[n][2] = {};
  char c[m] = {};
  short s[3][m] = {};
  g(a); g(p); g(c); g(s);
}
