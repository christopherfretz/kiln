// [expr.const]/10.9, [basic.life]/7, [dcl.init.aggr]/7: in a constexpr
// variable's initializer, the variable's name designates the object under
// construction, so a member (or element) already initialized may be read
// through it (its lifetime began within the evaluation), as clang and GCC
// accept. Aggregate elements are initialized in order. WP-1100.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
struct P { int x; int y; };
constexpr P p{1, p.x + 1};
static_assert(p.y == 2);
struct A { int v[2]; };
struct Q { const A &a; const int &r; };
constexpr Q q{A{{1, 2}}, q.a.v[1]};
static_assert(q.r == 2);
// A nested aggregate: its own earlier element, then the enclosing one's.
struct N { P a; int b; };
constexpr N n{{1, n.a.x + 1}, n.a.y + 1};
static_assert(n.a.y == 2 && n.b == 3);
// An array's later element reads an earlier one.
constexpr P arr[2] = {{1, 2}, {arr[0].y, 3}};
static_assert(arr[1].x == 2);
// Through a lambda, a function, a pointer to itself, a reference member.
constexpr P pl{1, [] { return pl.x + 1; }()};
static_assert(pl.y == 2);
extern const P pe;
constexpr int get() { return pe.x + 1; }
constexpr P pe{1, get()};
static_assert(pe.y == 2);
struct W { const W *self; int x; };
constexpr W w{&w, w.self == &w};
static_assert(w.x == 1);
struct R { int x; const int &r = x; int y = r + 1; };
constexpr R rr{4};
static_assert(rr.y == 5);
