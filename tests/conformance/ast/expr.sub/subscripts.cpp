//@ expect: ast
int pick(int *p, int a[], int i) {
  return p[i] + a[i + 1] + i[p] + "abc"[i];
}
