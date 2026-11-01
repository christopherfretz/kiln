// [expr.const]/5, [class.ctor] (WP-241): the "in call to" note of a
// constructor call spells the constructor and its argument values as
// clang does (`S(T{1})`), and at clang's location, for direct-list and
// copy-list-initialization (`S s = {T{1}}` at the `{`), a returned or
// passed braced list, an aggregate member and a converting argument.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:48 note "division by zero"
//@ diag: 50:23 note "in call to 'S(T{1})'"
//@ diag: 59:15 note "in call to 'a()'"
//@ diag: 59:35 error "static assertion expression is not an integral constant expression"
//@ diag: 46:48 note "division by zero"
//@ diag: 51:28 note "in call to 'S(T{1})'"
//@ diag: 59:35 note "in call to 'b()'"
//@ diag: 59:55 error "static assertion expression is not an integral constant expression"
//@ diag: 46:48 note "division by zero"
//@ diag: 49:28 note "in call to 'S(T{1})'"
//@ diag: 52:28 note "in call to 'ret()'"
//@ diag: 59:55 note "in call to 'c()'"
//@ diag: 59:75 error "static assertion expression is not an integral constant expression"
//@ diag: 46:48 note "division by zero"
//@ diag: 53:33 note "in call to 'S(T{1})'"
//@ diag: 59:75 note "in call to 'd()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:48 note "division by zero"
//@ diag: 54:27 note "in call to 'S(T{1})'"
//@ diag: 60:15 note "in call to 'e()'"
//@ diag: 60:35 error "static assertion expression is not an integral constant expression"
//@ diag: 46:48 note "division by zero"
//@ diag: 55:25 note "in call to 'S(T{1})'"
//@ diag: 60:35 note "in call to 'f()'"
//@ diag: 60:55 error "static assertion expression is not an integral constant expression"
//@ diag: 46:48 note "division by zero"
//@ diag: 56:28 note "in call to 'S(T{1})'"
//@ diag: 60:55 note "in call to 'g()'"
//@ diag: 60:75 error "static assertion expression is not an integral constant expression"
//@ diag: 46:48 note "division by zero"
//@ diag: 57:27 note "in call to 'S(T{1})'"
//@ diag: 60:75 note "in call to 'h()'"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:48 note "division by zero"
//@ diag: 58:33 note "in call to 'S(T{1})'"
//@ diag: 61:15 note "in call to 'i()'"
struct T { int v; };
struct S { int x; constexpr S(const T& t) : x(1/(t.v-1)) {} };
struct P { S s; };
constexpr int take(S s) { return s.x; }
constexpr S ret() { return {T{1}}; }
constexpr int a() { S s{T{1}}; return s.x; }
constexpr int b() { return S{T{1}}.x; }
constexpr int c() { return ret().x; }
constexpr int d() { return take({T{1}}); }
constexpr int e() { S s = {T{1}}; return s.x; }
constexpr int f() { P p{{T{1}}}; return p.s.x; }
constexpr int g() { P p = {T{1}}; return p.s.x; }
constexpr int h() { S s = T{1}; return s.x; }
constexpr int i() { return take(T{1}); }
static_assert(a()); static_assert(b()); static_assert(c()); static_assert(d());
static_assert(e()); static_assert(f()); static_assert(g()); static_assert(h());
static_assert(i());
