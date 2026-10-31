// [expr.prim.lambda.closure]/3-6: generic lambdas in constant evaluation.
// The call operator is a member function template (invented parameters
// for `auto`, an explicit template-parameter-list, constraints); each
// specialization captures through the closure built for the pattern.
template <class T> concept Small = sizeof(T) <= 4;
constexpr int f() {
  int k = 3;
  auto add = [k](auto x, auto y) { return x + y + k; };
  auto ref = [&k](auto d) { k += d; };
  ref(2);
  auto tp = []<class T>(T a, T b) { return a < b ? b : a; };
  auto va = [](auto... xs) { return (0 + ... + xs); };
  auto da = [](auto& x) -> decltype(auto) { return (x); };
  int z = 1;
  da(z) = 9;
  return add(1, 2) + tp(4, 2) + va(1, 2, 3, 4) + z + k;
}
static_assert(f() == 6 + 4 + 10 + 9 + 5);
constexpr auto sq = [](auto v) { return v * v; };
static_assert(sq(4) == 16 && sq(1.5) == 2.25);
constexpr int (*fp)(int) = sq;
static_assert(fp(5) == 25);
constexpr double (*dp)(double) = sq;
static_assert(dp(0.5) == 0.25);
auto cn = [](Small auto x) { return x + 1; };
static_assert(cn(1) == 2);
constexpr auto self = [](auto& me, int n) -> int { return n == 0 ? 1 : n * me(me, n - 1); };
static_assert(self(self, 5) == 120);
