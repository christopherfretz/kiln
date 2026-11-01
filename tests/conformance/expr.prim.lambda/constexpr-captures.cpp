// [expr.prim.lambda.capture]: captures in constant evaluation. A copy
// capture reads its closure member, a reference capture the variable it
// refers to; `mutable` lambdas update their own copies.
constexpr int copy(int a) { auto l = [a] { return a * 2; }; return l(); }
static_assert(copy(21) == 42);
constexpr int ref(int a) { auto l = [&a] { a += 1; }; l(); l(); return a; }
static_assert(ref(1) == 3);
constexpr int dflt(int a, int b) { return [=] { return a - b; }(); }
static_assert(dflt(5, 3) == 2);
constexpr int dref(int a) { auto l = [&] { return ++a; }; l(); return l() + a; }
static_assert(dref(0) == 4);
constexpr int initc(int a) { auto l = [x = a + 1, &y = a] { y = 10; return x; }; return l() + a; }
static_assert(initc(1) == 12);
constexpr int mut(int a) { auto l = [a]() mutable { return ++a; }; l(); int r = l(); return r * 10 + a; }
static_assert(mut(1) == 31);
constexpr int nest(int a) { return [&] { return [=] { return a + 1; }() + [&] { return ++a; }(); }() + a; }
static_assert(nest(1) == 6);
constexpr int konst() { const int n = 5; return [] { return n; }(); }
static_assert(konst() == 5);
struct P { int x, y; };
constexpr int agg(P p) { return [p] { return p.x * p.y; }(); }
static_assert(agg({6, 7}) == 42);
constexpr int arr() { int a[3] = {1, 2, 3}; auto l = [a] { return a[0] + a[2]; }; a[0] = 100; return l(); }
static_assert(arr() == 4);
