// [dcl.init.general]/16.5, /16.6.2.2, /9 (P0960; WP-182) in constant
// evaluation: a parenthesized initialization of a multidimensional array
// copy-list-initializes rows from braced-init-list arguments and
// value-initializes the rest (default member initializers, user-provided
// default constructors).
struct Q { int v = 3; };
struct R { constexpr R() : v(7) {} constexpr R(int x) : v(x) {} int v; };
constexpr int f() {
  Q q[2][2]({Q{1}});
  int z[2][3]({1, 2}, {3});
  R r[3][2]({R(1)}, {R(2), R(5)});
  return q[0][0].v * 1000000 + q[0][1].v * 100000 + q[1][1].v * 10000 + z[1][0] * 1000 +
         z[0][2] * 100 + r[1][1].v * 10 + r[2][0].v;
}
static_assert(f() == 1333057);
constexpr int g() {
  R (*p)[2] = new R[2][2]({R(4)});
  int s = p[0][0].v + p[0][1].v + p[1][1].v;
  delete[] p;
  return s;
}
static_assert(g() == 18);
