// [expr.new]/7, the accepted side of array-size-unknown-empty-parens.cpp:
// an omitted bound given by a braced list (also an empty one: an array of
// no elements), by a C++20 parenthesized list or a string literal, a
// written bound with `()`, and an array typedef with a bound (WP-1518).
//@ std: c++20
//@ expect: run
//@ stdout: 0 2 3 3 2 4
extern "C" int printf(const char *, ...);
typedef int A2[2];
int main() {
  int *a = new int[]{};
  int *b = new int[]{1, 2};
  int *c = new int[](1, 2, 3);
  char *d = new char[]("ab");
  int *e = new int[2]();
  int *f = new A2();
  int (*g)[2] = new int[][2]{{1, 2}, {3, 4}};
  printf("%d %d %d %d %d %d\n", e[0] + f[1], b[1], c[2], (int)sizeof("ab"), g[0][1], g[1][1]);
  delete[] a; delete[] b; delete[] c; delete[] d; delete[] e; delete[] f; delete[] g;
}
