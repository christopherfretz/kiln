//@ expect: ast
int loops(int n) {
  int s = 0;
  for (;;) {
    if (s > n) break;
    ++s;
  }
  for (int i = 0; i < n;) i += 2;
  for (; s; --s) continue;
  for (int j = 0, k = n; j < k; ++j, --k) s += j;
  return s;
}
