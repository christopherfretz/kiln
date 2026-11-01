// [expr.const]/5, [class.ctor] (WP-241): the "in call to" note of a
// constructor call spells the constructor and its argument values as
// clang does (`S(t)`, `S(T{1})`, `S({2}, 2)`, `R(r)`), for direct- and
// copy-initialization from a variable, a temporary and an array element.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:48 note "division by zero"
//@ diag: 38:31 note "in call to 'S(t)'"
//@ diag: 46:15 note "in call to 'a()'"
//@ diag: 46:35 error "static assertion expression is not an integral constant expression"
//@ diag: 36:48 note "division by zero"
//@ diag: 39:23 note "in call to 'S(T{1})'"
//@ diag: 46:35 note "in call to 'b()'"
//@ diag: 46:55 error "static assertion expression is not an integral constant expression"
//@ diag: 36:90 note "division by zero"
//@ diag: 40:23 note "in call to 'S({2}, 2)'"
//@ diag: 46:55 note "in call to 'c()'"
//@ diag: 46:75 error "static assertion expression is not an integral constant expression"
//@ diag: 37:48 note "division by zero"
//@ diag: 41:31 note "in call to 'R(r)'"
//@ diag: 46:75 note "in call to 'd()'"
//@ diag: 47:35 error "static assertion expression is not an integral constant expression"
//@ diag: 36:48 note "division by zero"
//@ diag: 43:35 note "in call to 'S(t)'"
//@ diag: 47:35 note "in call to 'f()'"
//@ diag: 47:55 error "static assertion expression is not an integral constant expression"
//@ diag: 36:48 note "division by zero"
//@ diag: 44:42 note "in call to 'S(arr[1])'"
//@ diag: 47:55 note "in call to 'g()'"
//@ diag: 47:75 error "static assertion expression is not an integral constant expression"
//@ diag: 36:48 note "division by zero"
//@ diag: 45:27 note "in call to 'S(T{1})'"
//@ diag: 47:75 note "in call to 'h()'"
struct T { int v; };
struct S { int x; constexpr S(const T& t) : x(1/(t.v-1)) {} constexpr S(T a, int b) : x(1/(a.v-b)) {} };
struct R { int x; constexpr R(const R& r) : x(1/(r.x-1)) {} constexpr R(int a) : x(a) {} };
constexpr int a() { T t{1}; S s(t); return s.x; }
constexpr int b() { S s(T{1}); return s.x; }
constexpr int c() { S s(T{2}, 2); return s.x; }
constexpr int d() { R r(1); R q(r); return q.x; }
constexpr int e() { R q(R(1)); return q.x; }
constexpr int f() { T t{1}; S s = t; return s.x; }
constexpr int g() { T arr[2]{{0},{1}}; S s(arr[1]); return s.x; }
constexpr int h() { S s = {T{1}}; return s.x; }
static_assert(a()); static_assert(b()); static_assert(c()); static_assert(d());
static_assert(e()); static_assert(f()); static_assert(g()); static_assert(h());
