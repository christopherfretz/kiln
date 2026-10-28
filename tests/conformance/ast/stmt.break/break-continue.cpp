//@ expect: ast
int scan(int *a, int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) {
    if (a[i] < 0) continue;
    if (a[i] == 0) break;
    s += a[i];
  }
  while (1) { break; }
  do { continue; } while (0);
  return s;
}
