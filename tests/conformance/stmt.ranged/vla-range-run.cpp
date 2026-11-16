//@ expect: run
//@ strict-diags
//@ args: -std=gnu++20
//@ stdout: 10 10 6 9 4 2 3
// [stmt.ranged]/1.1 over a variable length array (GNU extension): the end
// is `__range + sizeof(__range) / sizeof(element)` (clang
// `BuildCXXForRangeStmt`), the size the array was created with, so
// changing the bound's variable afterwards does not change the count, and
// the bound expression is not evaluated again. Found running GCC's C++
// testsuite.
extern "C" int printf(const char *, ...);

int calls;
int bound(int n) { ++calls; return n; }

int fill_then_sum(int n) {
  int a[n];
  int k = 0;
  for (int &x : a) x = ++k;
  n = 100;
  int s = 0;
  for (int x : a) s += x;
  return s;
}
int skip_and_stop(int n) {
  int a[n];
  for (auto &&x : a) x = 2;
  int s = 0;
  for (const auto &x : a) {
    if (x == 0) continue;
    s += x;
    if (s >= 10) break;
  }
  return s;
}
int rows(int n, int m) {
  int a[n][m];
  int s = 0;
  for (auto &row : a)
    for (int &x : row) x = 1;
  for (auto &row : a)
    for (int x : row) s += x;
  return s;
}
template <class T> T templ(int n) {
  T a[n];
  T s = 0;
  for (T &x : a) x = 3;
  for (auto x : a) s += x;
  return s;
}
int evaluated_once(int n) {
  int a[bound(n)];
  int c = 0;
  for (int x : a) { (void)x; ++c; }
  return c;
}

int main() {
  int a = fill_then_sum(4);
  int b = skip_and_stop(9);
  int c = rows(2, 3);
  int d = templ<int>(3);
  int e = (int)templ<long>(2) - 2;
  int f = evaluated_once(2);
  printf("%d %d %d %d %d %d %d\n", a, b, c, d, e, f, calls + 2);
}
