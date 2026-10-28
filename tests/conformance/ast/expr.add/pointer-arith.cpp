//@ expect: ast
long diff(int *a, int *b, int n) {
  int *mid = a + n / 2;
  mid = 1 + mid;
  mid -= 1;
  return b - mid;
}
