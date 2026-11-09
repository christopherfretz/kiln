// [expr.const]/10.9, [basic.life]/7: in a constexpr variable's initializer,
// a member that is already initialized may be read through the variable's
// name (its lifetime began with its own initialization), as clang does.
//@ std: c++26
//@ xfail(kiln): WP-1018 follow-up, the variable's name designates its not-yet-live static allocation, not the object under construction
//@ expect: compile-ok
struct P { int x; int y; };
constexpr P p{1, p.x + 1};
static_assert(p.y == 2);
struct A { int v[2]; };
struct Q { const A &a; const int &r; };
constexpr Q q{A{{1, 2}}, q.a.v[1]};
static_assert(q.r == 2);
