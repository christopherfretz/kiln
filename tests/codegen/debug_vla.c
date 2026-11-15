// Debug info (WP-1445): C variable-length arrays, a local and a
// parameter: the local's DISubrange counts are `__vla_exprN` variables;
// a VLA parameter is a pointer.
//@ args: -g -std=gnu17
long g(int n, int m, int a[n][m]) {
  int vla[n];
  long v2[n][m];
  vla[0] = a[0][0];
  v2[0][0] = 2;
  return vla[0] + v2[0][0];
}
