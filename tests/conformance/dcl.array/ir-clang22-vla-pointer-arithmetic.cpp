// [expr.add]/4, [expr.pre.incr], [dcl.array] (variable length arrays are
// a GNU extension): arithmetic on a pointer to a variable length array
// steps over whole rows. clang's `emitPointerArithmetic` scales the index
// by the row's element count (`mul nsw`), `++`/`--` step by that count
// (negated with `sub nsw 0, n` for `--`) and a pointer difference divides
// by the row's size in bytes (spec/codegen.md §3 "GEP flags"). Compared
// with clang 22's IR exactly.
//@ expect: ir
//@ requires: clang>=22
//@ args: -std=gnu++26
long rows(int n, int i) {
  int a[4][n];
  auto p = a + 1;
  auto q = a + 3;
  p++;
  --q;
  p += i;
  q -= i;
  return (p - q) + p[i][0];
}
long planes(int n, int m, long i) {
  double a[2][n][m];
  auto p = a;
  auto r = p + i;
  auto s = i + p;
  r--;
  ++s;
  return s - r;
}
long fixed_inner(int n, unsigned i) {
  char a[5][n][3];
  auto p = a + 4;
  auto r = p - i;
  r++;
  return r - p;
}
