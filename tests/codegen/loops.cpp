// for / while / do-while with break and continue, and nested loops.
int loops(int n) {
  int s = 0;
  for (int i = 0; i < n; i++) {
    if (i % 3 == 0) continue;
    for (int j = i; j > 0; j /= 2) s += j;
  }
  int k = n;
  while (k--) { if (s > 1000) break; s++; }
  do { s -= 2; } while (s > 500);
  return s;
}
