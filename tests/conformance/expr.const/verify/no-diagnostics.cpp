// [expr.const]/5, [expr.const]/13 (WP-143): a constexpr function whose
// evaluation stays within the core constant expression rules: loops,
// mutation of local objects, and lifetime-bounded temporaries.
//@ verify
// expected-no-diagnostics
constexpr int sum(int n) {
  int s = 0;
  for (int i = 1; i <= n; ++i)
    s += i;
  return s;
}
static_assert(sum(10) == 55);
struct P { int a, b; };
constexpr P swap(P p) { return {p.b, p.a}; }
static_assert(swap({1, 2}).a == 2);
constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
static_assert(fib(20) == 6765);
