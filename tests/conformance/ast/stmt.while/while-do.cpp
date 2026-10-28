//@ expect: ast
int count(int n) {
  int c = 0;
  while (int m = n - c) {
    if (m == 2) continue;
    ++c;
  }
  do {
    c--;
  } while (c > 0);
  while (true) break;
  return c;
}
