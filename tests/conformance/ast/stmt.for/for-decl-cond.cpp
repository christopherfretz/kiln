//@ expect: ast
int sum(int n) {
  int s = 0;
  for (int i = 0, j = n; i < j; ++i, --j) s += i * j;
  for (; n;) --n;
  for (int k = 3; int left = k; --k) s += left;
  for (;;) break;
  return s;
}
