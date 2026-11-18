// [dcl.constexpr]/3 since C++20 (P1331R2): a constexpr function may
// define a variable without initializing it, so long as it is assigned
// before it is read in a constant evaluation; no diagnostic, and a
// lambda defining one stays implicitly constexpr
// ([expr.prim.lambda.closure]/5). Found running GCC's C++ testsuite.
//@ args: -std=c++20
//@ strict-diags

constexpr int copy(int i) { int j; j = i; return j; }
static_assert(copy(3) == 3);
constexpr int sum(int n) {
  int a[4];
  for (int k = 0; k < 4; ++k) a[k] = k * n;
  int s;
  s = 0;
  for (int i; ;) { i = 0; s += a[3] + i; break; }
  return s;
}
static_assert(sum(2) == 6);
struct Box {
  int v;
  constexpr Box(int i) : v(0) { int q; q = i; v = q; }
};
static_assert(Box(5).v == 5);
template <class T> constexpr T tcopy(T i) { T j; j = i; return j; }
static_assert(tcopy(7) == 7);
constexpr int via_lambda = [](int i) { int j; j = i; return j; }(4);
static_assert(via_lambda == 4);
