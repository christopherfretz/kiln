//@ expect: ast
bool cmp(int a, unsigned u, int *p, int *q, double d) {
  return a < 3 && u >= 2u && p != q && p == nullptr && d > a && p <= q;
}
