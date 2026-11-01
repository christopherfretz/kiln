// "in call to" notes for closure calls (WP-237): clang's `describeCall`
// names the object expression and the call operator (`f.operator()(0)`),
// a generic lambda's specialization (`gl.operator()<int>(0)`), a static
// call operator without an object, and the static invoker with the first
// argument as the object (`(0->operator()()`). [expr.prim.lambda.closure],
// [expr.const]/10.
//@ expect: compile-fail
//@ args: -std=c++23
//@ strict-diags
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:34 note "division by zero"
//@ diag: 44:10 note "in call to 'f.operator()(0)'"
//@ diag: 46:15 note "in call to 'g()'"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:65 note "division by zero"
//@ diag: 47:80 note "in call to 'f2.operator()(0)'"
//@ diag: 48:15 note "in call to 'h()'"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:42 note "division by zero"
//@ diag: 50:15 note "in call to 'lf.operator()(0)'"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 51:74 note "division by zero"
//@ diag: 51:89 note "in call to 'q.operator()()'"
//@ diag: 52:15 note "in call to 'S{0}.run()'"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:55 note "division by zero"
//@ diag: 53:70 note "in call to 'gl.operator()<int>(0)'"
//@ diag: 54:15 note "in call to 'gen()'"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:59 note "division by zero"
//@ diag: 55:74 note "in call to 'operator()(0)'"
//@ diag: 56:15 note "in call to 'st()'"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:53 note "division by zero"
//@ diag: 57:87 note "in call to '(0->operator()()'"
//@ diag: 58:15 note "in call to 'inv()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:76 note "division by zero"
//@ diag: 59:91 note "in call to 'inner.operator()(0)'"
//@ diag: 59:111 note "in call to 'outer.operator()()'"
//@ diag: 60:15 note "in call to 'nest()'"
constexpr int g() {
  auto f = [](int x) { return 10 / x; };
  return f(0);
}
static_assert(g() == 0);
constexpr int h() { int k = 0; auto f2 = [&k](int x) { return k / x; }; return f2(0); }
static_assert(h() == 0);
constexpr auto lf = [](int x) { return 1 / x; };
static_assert(lf(0));
struct S { int m; constexpr int run() const { auto q = [this] { return 1 / m; }; return q(); } };
static_assert(S{0}.run());
constexpr int gen() { auto gl = [](auto x) { return 1 / x; }; return gl(0); }
static_assert(gen());
constexpr int st() { auto s = [](int x) static { return 1 / x; }; return s(0); }
static_assert(st());
constexpr int inv() { auto l = [](int x) { return 1 / x; }; int (*p)(int) = l; return p(0); }
static_assert(inv());
constexpr int nest() { auto outer = [] { auto inner = [](int x) { return 1 / x; }; return inner(0); }; return outer(); }
static_assert(nest());
