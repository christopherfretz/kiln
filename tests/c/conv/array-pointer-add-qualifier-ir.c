// C23 6.7.3p9, C17 6.5.16.1p1: the accepted `T (*)[N]` to
// `const T (*)[N]` conversions (array-pointer-add-qualifier-*.c) are
// no-op pointer conversions and lower as clang's: initialization,
// assignment, argument, return, comparison, a VLA pointee. WP-1427.
//@ args: -std=gnu17
//@ expect: ir
typedef unsigned int Buckets[256];
unsigned int sum(Buckets *h) { const Buckets *const c = h; return (*c)[0] + (*c)[255]; }
void take(const int (*p)[3]);
const int (*ret(int (*p)[3]))[3] { return p; }
int f(int n, int (*a)[3], int (*m)[2][3], int (*v)[n]) {
  const int (*ca)[3] = a;
  ca = a + 1;
  take(a);
  const int (*cm)[2][3] = m;
  const int (*cv)[n] = v;
  return (*ca)[1] + (*cm)[1][2] + cv[1][0] + (a == ca) + (*ret(a))[2];
}
