// [expr.prim.lambda.closure]/5: the function call operator of a lambda is
// constexpr if it satisfies the requirements of a constexpr function.
constexpr auto sq = [](int x) { return x * x; };
static_assert(sq(5) == 25);
static_assert([] { return 42; }() == 42);
static_assert([](int a, int b) -> long { return a + b; }(1, 2) == 3L);
constexpr int sum(int n) {
  auto l = [](int k) { int s = 0; for (int i = 0; i < k; ++i) s += i; return s; };
  return l(n);
}
static_assert(sum(10) == 45);
struct S { int v; };
static_assert([](S s) { return s.v; }(S{7}) == 7);
static_assert([](const int& r) { return r + 1; }(1) == 2);
