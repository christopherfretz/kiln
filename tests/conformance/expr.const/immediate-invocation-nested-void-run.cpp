// [expr.const]/16-18 (P2564R3): a call to a void consteval function in a
// discarded-value expression inside a constexpr function template
// specialization or a lambda makes the enclosing function
// immediate-escalating, so the call is not an immediate invocation of
// its own: it is evaluated in place, as part of the enclosing constant
// evaluation, and its side effects on the caller's locals are seen there
// (spec/oven.md "Nested immediate invocations"; WP-537). Covers a plain
// call, a comma fold over a pack in a lambda initializing a variable
// template, a call in a template for body, and the value-returning forms
// that already worked. The escalated specializations are immediate
// functions, so they are only called in constant expressions.
//@ expect: run
//@ strict-diags
//@ stdout: 1 1 2 6 5
extern "C" int printf(const char*, ...);

consteval void f(int& x) { ++x; }
template <int N> constexpr int g() { int x = N; f(x); return x; }
static_assert(g<0>() == 1);

struct C { int n = 0; };
consteval void defect(int r, C& c) { if (r > 0) ++c.n; }
template <int... Rs>
inline constexpr C claim_v = [] { C c{}; (defect(Rs, c), ...); return c; }();
static_assert(claim_v<1, 0>.n == 1);
static_assert(claim_v<1, 0, 3>.n == 2);

// The same call through a parenthesized, cast-to-void and comma operand.
template <int N> constexpr int g2() {
  int x = N;
  (f(x));
  (void)f(x);
  f(x), f(x);
  return x;
}
static_assert(g2<2>() == 6);

// The value-returning forms evaluate in place too.
consteval int inc(int& x) { return ++x; }
template <int... Rs> constexpr int sum() { int s = 0, c = 0; ((s += inc(c) + Rs), ...); return s; }
static_assert(sum<1, 1>() == 5);

int main() {
  constexpr int a = g<0>();
  constexpr int b = g2<2>();
  constexpr int s = sum<1, 1>();
  printf("%d %d %d %d %d\n", a, claim_v<1, 0>.n, claim_v<1, 0, 3>.n, b, s);
  return 0;
}
