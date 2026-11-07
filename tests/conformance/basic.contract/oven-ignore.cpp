// [basic.contract.eval]/1, [expr.const] (P2900R14): under the ignore
// semantic a contract assertion reached during constant evaluation is not
// evaluated, so even a violated one leaves the evaluation a constant
// expression. GCC 16.1 agrees.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ args: -fcontract-evaluation-semantic=ignore
constexpr int f(const int x) pre(x > 100) post(r: r < 0) { return x + 1; }
constexpr int g(int x) { contract_assert(x > 100); return x * 2; }
struct S { int m; constexpr int get() const pre(m > 9) { return m; } };
template <class T> constexpr T t(T v) pre(v != v) { return v; }
static_assert(f(1) == 2);
static_assert(g(2) == 4);
static_assert(S{3}.get() == 3);
static_assert(t(5) == 5);
constexpr auto lam = [](const int a) post(r: r != a) { return a; };
static_assert(lam(7) == 7);
