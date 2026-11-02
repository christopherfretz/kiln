//@ expect: run
//@ strict-diags
//@ args: -std=gnu++26 -Wno-pointer-arith
//@ stdout: 1 3 0 22 11 20
//@ stdout: 6 1
// [expr.add]/4, [expr.pre.incr], [expr.post.incr] on pointers to variable
// length arrays (a GNU extension, [dcl.array]): `p + i`, `++p`, `p--`,
// `p += i`, `p -= i` move by whole rows of the run-time row length and
// `p - q` counts rows (spec/codegen.md §3). clang 19 warns that the
// subtraction's pointee has zero size (-Wpointer-arith; clang 22 does not).
extern "C" int printf(const char *, ...);

int main() {
  int n = 3;
  int a[4][n];
  for (int r = 0; r < 4; ++r)
    for (int c = 0; c < n; ++c) a[r][c] = r * 10 + c;
  auto p = a;
  auto q = a + 3;
  ++p;
  long d1 = p - a;
  long d2 = q - a;
  q--;
  p += 2;
  p -= 1;
  printf("%ld %ld %ld %d %d %d\n", d1, d2, (long)(p - q), p[0][2], (*(a + 1))[1], q[0][0]);
  char b[2][n][2];
  auto r = b + 2;
  r--;
  long d3 = r - b;
  printf("%ld %ld\n", (long)sizeof(*r), d3);
  return 0;
}
