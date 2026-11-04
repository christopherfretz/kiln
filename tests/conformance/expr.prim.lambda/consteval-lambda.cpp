// [expr.prim.lambda.closure]/5, [expr.const]/18-19: a consteval lambda's
// call operator is an immediate function; a constexpr lambda that calls
// a consteval function with a non-constant argument is
// immediate-escalating; a call outside an immediate context must be a
// constant expression. (clang 19 names the closure "(anonymous class)".)
//@ expect: compile-fail
//@ diag: 15:23 error "call to consteval function"
//@ diag: 18:23 error "call to immediate function"
//@ diag: 19:67 error "call to consteval function 'h(int)::"
constexpr auto sq = [](int n) consteval { return n * n; };
static_assert(sq(4) == 16);
consteval int twice(int n) { return 2 * n; }
constexpr auto el = [](int n) { return twice(n); };
static_assert(el(5) == 10);
int f(int v) { return sq(v); }
int ok = el(6) + sq(2);
auto p = &sq;
int g(int v) { return el(v); }
int h(int v) { auto l = [](int n) consteval { return n; }; return l(v); }
