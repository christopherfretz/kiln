// [expr.type.conv]/2, [dcl.init.list]/3.10, [class.temporary]/6,
// [expr.const]/4: array prvalues from a braced list (a functional cast to
// an array type, or a list bound to an array reference) are evaluated as
// temporaries; their element destructors run at the end of the
// full-expression, and a const one extended by a constexpr reference is
// usable in constant expressions.
struct T { int v; constexpr T(int x) : v(x) {} constexpr ~T() {} };
using A = int[3];
using TA = T[2];
using U = int[];
constexpr int sum(const int (&a)[3]) { return a[0] + a[1] + a[2]; }
static_assert(sum({1, 2, 3}) == 6);
static_assert(A{4, 5, 6}[2] == 6);
static_assert(U{4, 5, 6, 7}[3] == 7);
static_assert(TA{8, 9}[1].v == 9);
constexpr int ce() {
  int (&&r)[2] = {1, 2};
  const TA &t = TA{3, 4};
  return r[0] + r[1] + t[1].v;
}
static_assert(ce() == 7);
constexpr int tsum(const T (&a)[2]) { return a[0].v + a[1].v; }
static_assert(tsum({10, 20}) == 30);
constexpr int cnt() {
  int n = 0;
  struct C { int *p; constexpr ~C() { ++*p; } };
  using CA = C[3];
  (void)CA{&n, &n, &n};
  return n;
}
static_assert(cnt() == 3);
constexpr const int (&gr)[2] = {11, 12};
static_assert(gr[1] == 12);
using B = int[2];
constexpr const B &gr2 = B{13, 14};
static_assert(gr2[1] == 14);
