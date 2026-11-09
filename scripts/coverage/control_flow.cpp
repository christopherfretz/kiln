// Coverage comparison program (scripts/coverage-compare.py): every
// statement form with a counter, &&/|| and ?: in each emitter.
#include <cstdio>
static int st(int x) { return x ? x + 1 : 0; }
inline int il(int a, int b) { return a && b; }
int loops(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) {
    if (i % 3 == 0) continue;
    if (i > 50) break;
    s += i;
  }
  for (;;) { if (s > 10) break; s++; }
  while (n-- > 0) s ^= n;
  do { s += 2; } while (s < 5);
  int arr[] = {1, 2, 3};
  for (int v : arr) s += v;
  return s;
}
int sw(int x) {
  int r = 0;
  switch (x) {
  case 0:
  case 1: r = 1; break;
  case 2: r = 2; [[fallthrough]];
  case 3 ... 5: r += 3; break;
  default: r = -1;
  }
  switch (x) { case 7: return 7; }
  return r;
}
int logic(int a, int b, int c) {
  int r = 0;
  if (a && (b || c)) r++;
  if (!(a || b) && c) r += 2;
  r += (a && b) || c;
  bool t = a > 1 && b < 2;
  while (a > 0 && b > 0) { a--; b--; }
  r += a ? b : c;
  r += (a > b) ? (b ? 1 : 2) : 3;
  if (1 && b) r++;
  if (0 || c) r++;
  if (a && 1) r++;
  r += true && b;
  r += false || c;
  return r + t;
}
int g(int x) {
  if (x < 0) goto neg;
  return x;
neg:
  return -x;
}
int tc(int x) {
  try {
    if (x) throw x;
  } catch (int e) {
    return e;
  } catch (...) {
    return -1;
  }
  return 0;
}
int &lv(bool b, int &x, int &y) { return b ? x : y; }
struct P { int a, b; };
P agg(bool b) { P x{1, 2}, y{3, 4}; return b ? x : y; }
_Complex double cx(bool b, _Complex double x) { return b ? x : 0.0; }
int main() {
  int s = loops(10) + sw(1) + sw(3) + sw(9) + sw(7) + logic(1, 0, 1) + logic(0, 0, 1) + g(-3) + g(4) + tc(0) + tc(5) + st(1) + il(1, 1);
  int x = 1, y = 2;
  lv(true, x, y) = 5;
  s += agg(false).a + (int)__real__ cx(true, 2.0);
  printf("%d\n", s);
  return 0;
}
