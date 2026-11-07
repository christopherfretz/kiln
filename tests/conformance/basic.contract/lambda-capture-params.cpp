// [expr.prim.lambda.capture]/3.3, [basic.pre]/9, [expr.prim.id.unqual]/7
// (P2900R14): a lambda inside a function's precondition or postcondition
// captures that function's parameters and the result binding (a local
// entity), by copy and by reference, explicitly and with [=]/[&], in a
// template, on a later definition, in a member, a lambda and a generic
// lambda; the constant evaluator sees the captured values and reports a
// violation through the capture. GCC 16.1 rejects naming a parameter in
// such a lambda ("use of parameter outside function body"); kiln follows
// the wording (spec/contracts.md §9), so there is no oracle.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ args: -fcontract-evaluation-semantic=enforce
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:33 note "contract predicate is false in constant expression"
//@ diag: 46:15 note "in call to 'decl(-1)'"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:38 note "contract predicate is false in constant expression"
//@ diag: 47:15 note "in call to 'gl.operator()<int>(-2)'"
template <class T> constexpr T tf(const T a, const T& b)
  pre([a, &b] { return a < b; }()) post(r: [=] { return r == a + b; }()) { return a + b; }
constexpr int decl(const int a) pre([&] { return a > 0; }())
  post(r: [r, a] { return r == a * 2; }());
constexpr int decl(const int x) { return x * 2; }
struct S {
  int m = 2;
  constexpr int h(const int k) const pre([this, k] { return m < k; }())
    post(r: [&] { return r == k + m; }()) { return k + m; }
  constexpr int f(const int k) const pre([k, this] { return k > m; }());
};
constexpr int S::f(const int k) const { return k; }
constexpr int mut(int a) pre([=]() mutable { ++a; return a > 1; }()) { return a; }
constexpr int& refret(int& x) post(r: [&r, &x] { return &r == &x; }()) { return x; }
constexpr bool t() { int v = 0; return &refret(v) == &v; }
template <class T> struct C { constexpr T f(const T a) pre([&] { return a > 0; }()) { return a; } };
static_assert(tf(1, 2) == 3);
static_assert(decl(4) == 8);
static_assert(S{}.h(5) == 7 && S{}.f(3) == 3);
static_assert(mut(1) == 1);
static_assert(t());
static_assert(C<int>{}.f(2) == 2);
constexpr auto lam = [](const int a) pre([a] { return a > 0; }()) post(r: [&] { return r == a; }()) { return a; };
constexpr auto gl = [](const auto a) pre([a] { return a > 0; }()) { return a; };
static_assert(lam(1) == 1);
static_assert(gl(2) == 2);
static_assert(decl(-1) == -2);
static_assert(gl(-2) == -2);
