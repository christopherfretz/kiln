// [over.ics.list]/8, [dcl.init.aggr]/4-5, /16: an aggregate parameter is
// viable for a braced-init-list only if aggregate initialization from it
// would succeed, so the list is walked under brace elision: a nested braced
// list initializes one element whole (an array member from `{}`, leaving
// the next clause for the next member), an expression that does not
// convert to a subaggregate starts brace elision, every clause must
// convert to the element it initializes, and an element left over must be
// initializable from `{}` (no reference member without a default member
// initializer, no class without a default constructor).
//@ expect: compile-ok
//@ strict-diags
struct E {};
struct B { E e[2]; int x; };
struct B2 { int a, b, c; };
constexpr int m(B) { return 1; }
constexpr int m(B2) { return 2; }
static_assert(m({{}, {}, 1}) == 2);

struct R { int& r; };
struct R2 { int a; };
constexpr int q(R) { return 1; }
constexpr int q(R2) { return 2; }
static_assert(q({}) == 2);

struct P { int* p; };
struct P2 { long l; };
constexpr int p(P) { return 1; }
constexpr int p(P2) { return 2; }
static_assert(p({1}) == 2);

struct Q { int a[2]; int b; };
struct Q2 { E a; int b; int c; };
constexpr int u(Q) { return 1; }
constexpr int u(Q2) { return 2; }
static_assert(u({1, 2, 3}) == 1);
static_assert(u({{1, 2}, 3}) == 1);
static_assert(u({{}, 2, 3}) == 2);

struct In { int a, b; };
struct N { In in; int c; };
struct N2 { int a; E b; int c; };
constexpr int v(N) { return 1; }
constexpr int v(N2) { return 2; }
static_assert(v({1, 2, 3}) == 1);

struct X { constexpr X(int) {} };
struct Y { X x; int y; };
struct Y2 { E e; int y; };
constexpr int y(Y) { return 1; }
constexpr int y(Y2) { return 2; }
static_assert(y({1, 2}) == 1);
static_assert(y({{}, 2}) == 2);

struct Z { int a; X x; };
struct Z2 { int a; };
constexpr int z(Z) { return 1; }
constexpr int z(Z2) { return 2; }
static_assert(z({1}) == 2);
